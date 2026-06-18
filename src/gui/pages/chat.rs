//! 3-panel chat page matching the website layout.
//!
//! LEFT:   Collapsible DMs (red), Groups (green), Servers (blue), Connection settings
//! MIDDLE: Channel header, message feed, input bar
//! RIGHT:  Friends list, Server members list

use egui::{Align, Color32, Frame, Layout, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::{ChatMessage, ChatUser, GuiState};
use crate::gui::theme::Theme;
use crate::gui::widgets;

// Maximum messages kept in the local chat buffer (was hardcoded, now uses theme.max_messages if needed).

/// Minimum panel width in points.
const MIN_PANEL_WIDTH: f32 = 150.0;
/// Maximum panel width in points.
const MAX_PANEL_WIDTH: f32 = 400.0;

// Section tint colors now come from theme.ron (theme.dm_bg(), theme.group_bg(), theme.server_bg(), etc.)

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // ── Clipboard image paste detection ──
    // The Ctrl+V key event is detected at the RAW WINIT LAYER (see
    // src/lib.rs window_event) which sets state.pending_clipboard_paste.
    // We CANNOT detect it through egui's input here because egui-winit
    // intercepts the paste shortcut: it reads clipboard TEXT only and
    // returns early WITHOUT emitting a V key event, so for an image
    // clipboard egui's input sees neither a key event nor a paste event.
    // v0.232 (in-input check) + v0.233 (top-of-draw egui check) both
    // failed for exactly this reason. v0.234 reads the winit-set flag.
    //
    // No focus check needed: if there's an image on the clipboard and
    // the user pressed Ctrl+V on the chat page, they almost certainly
    // want it uploaded to the active channel (same as Discord/Slack).
    // Text-only clipboards return None from try_grab_clipboard_image_as_png
    // so egui's TextEdit handles regular text paste normally.
    let ctrl_v_pressed = std::mem::take(&mut state.pending_clipboard_paste);
    if ctrl_v_pressed {
        if let Some(png_bytes) = try_grab_clipboard_image_as_png() {
            // Grab the PNG on the main thread (clipboard access), but run the
            // (potentially seconds-long) network upload on a WORKER thread so a
            // big paste doesn't freeze the UI. The drain block below sends the
            // chat message with the returned URL once the upload finishes.
            let server = state.server_url.clone();
            let pk = state.profile_public_key.clone();
            let channel = state.chat_active_channel.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let result = upload_image_png_blocking(&server, &pk, png_bytes)
                    .map_err(|e| e.to_string());
                let _ = tx.send(result);
            });
            state.clipboard_upload = Some((channel, rx));
        }
        // If no image on clipboard, fall through — egui's TextEdit
        // sees the Ctrl+V key event normally and handles text paste.
    }

    // Drain a finished clipboard-image upload: on success, send the chat
    // message carrying the image URL (ws + Dilithium sign on the main thread).
    if let Some((channel, rx)) = state.clipboard_upload.as_ref() {
        match rx.try_recv() {
            Ok(Ok(url)) => {
                let channel = channel.clone();
                state.clipboard_upload = None;
                if let Some(ref client) = state.ws_client {
                    if client.is_connected() {
                        let ts = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        let mut m = serde_json::json!({
                            "type": "chat",
                            "from": state.profile_public_key,
                            "from_name": state.user_name,
                            "content": url,
                            "timestamp": ts,
                            "channel": channel,
                        });
                        // Inc2.MED-1: sign with Dilithium3 over `content\ntimestamp`
                        // (relay requires pq_signature for non-bot chat).
                        if let Some(seed) = state.private_key_bytes.as_ref() {
                            m["pq_signature"] = serde_json::Value::String(
                                crate::net::identity::pq_sign_chat(seed, &url, ts),
                            );
                        }
                        client.send(&m.to_string());
                        log::info!("Clipboard image uploaded and sent to {}", channel);
                    }
                }
            }
            Ok(Err(e)) => {
                state.clipboard_upload = None;
                log::warn!("Clipboard image upload failed: {e}");
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.clipboard_upload = None;
                log::warn!("Clipboard image upload worker stopped unexpectedly");
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(120));
            }
        }
    }

    // ── LEFT PANEL ──
    let left_panel = egui::SidePanel::left("chat_left_panel")
        .frame(Frame::NONE.fill(theme.bg_sidebar_dark()).inner_margin(0.0))
        .width_range(MIN_PANEL_WIDTH..=MAX_PANEL_WIDTH);
    let left_panel = if state.chat_left_panel_locked {
        left_panel.exact_width(state.chat_left_panel_width).resizable(false)
    } else {
        left_panel.default_width(state.chat_left_panel_width).resizable(true)
    };
    // Panel lock buttons MOVED to center panel header in v0.188.0.
    // The sidebar panels just render their content; the lock toggle is a
    // small button on the chat header alongside the channel name. This
    // keeps the sidebars clean and puts the controls where the user is
    // most likely already looking.
    let left_response = left_panel.show(ctx, |ui| {
        draw_left_panel(ui, theme, state);
    });
    if !state.chat_left_panel_locked {
        state.chat_left_panel_width = left_response.response.rect.width();
    }

    // ── RIGHT PANEL ──
    let right_panel = egui::SidePanel::right("chat_right_panel")
        .frame(Frame::NONE.fill(theme.bg_sidebar_dark()).inner_margin(0.0))
        .width_range(MIN_PANEL_WIDTH..=MAX_PANEL_WIDTH);
    let right_panel = if state.chat_right_panel_locked {
        right_panel.exact_width(state.chat_right_panel_width).resizable(false)
    } else {
        right_panel.default_width(state.chat_right_panel_width).resizable(true)
    };
    let right_response = right_panel.show(ctx, |ui| {
        draw_right_panel(ui, theme, state);
    });
    if !state.chat_right_panel_locked {
        state.chat_right_panel_width = right_response.response.rect.width();
    }

    // ── CENTER PANEL ──
    egui::CentralPanel::default()
        .frame(Frame::NONE.fill(theme.bg_panel()).inner_margin(0.0))
        .show(ctx, |ui| {
            draw_center_panel(ui, theme, state);
        });

    // ── FLOATING LOCK OVERLAYS (v0.190.0) ──
    // Pinned to the EXACT top corners of the center panel with zero
    // inner padding. The button is 14×14 and sits flush against the
    // panel boundary — left button at the left panel's right edge,
    // right button such that its right edge meets the right panel's
    // left edge. The Area gets `Frame::NONE` (no inner margin) and
    // the button helper itself avoids any horizontal wrapper / spacing,
    // so what you see on screen is exactly 14 pixels of icon and
    // nothing else.
    const LOCK_PX: f32 = 14.0;
    let left_panel_right = left_response.response.rect.right();
    let right_panel_left = right_response.response.rect.left();
    let header_top = left_response.response.rect.top();

    egui::Area::new(egui::Id::new("chat_left_lock_overlay"))
        .fixed_pos(egui::pos2(left_panel_right, header_top))
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            // Strip the parent style's item_spacing inside this Area
            // so even nested allocations don't introduce stray gaps.
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            if draw_panel_lock_button(ui, theme, state.chat_left_panel_locked) {
                state.chat_left_panel_locked = !state.chat_left_panel_locked;
                crate::config::AppConfig::from_gui_state(state).save();
            }
        });
    egui::Area::new(egui::Id::new("chat_right_lock_overlay"))
        .fixed_pos(egui::pos2(right_panel_left - LOCK_PX, header_top))
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            if draw_panel_lock_button(ui, theme, state.chat_right_panel_locked) {
                state.chat_right_panel_locked = !state.chat_right_panel_locked;
                crate::config::AppConfig::from_gui_state(state).save();
            }
        });

    // ── USER PROFILE MODAL ──
    if state.chat_user_modal_open {
        draw_user_modal(ctx, theme, state);
    }

    // ── UNENCRYPTED-DM CONFIRMATION MODAL (v0.199.0, B3 fix) ──
    // Pops up when the user clicked Send on a DM that we couldn't
    // encrypt (recipient ECDH key missing, etc.). User must explicitly
    // confirm "Send unencrypted" or cancel — no silent plaintext fallback.
    draw_unencrypted_dm_modal(ctx, theme, state);

    // ── CREATE CHANNEL MODAL ──
    if state.show_create_channel_modal {
        draw_create_channel_modal(ctx, theme, state);
    }

    // ── ADD SERVER MODAL (v0.187.0) ──
    if state.show_add_server_modal {
        draw_add_server_modal(ctx, theme, state);
    }

    // ── EDIT CHANNEL MODAL ──
    if state.show_channel_edit_modal {
        draw_edit_channel_modal(ctx, theme, state);
    }

    // ── CREATE GROUP MODAL ──
    if state.show_create_group_modal {
        draw_create_group_modal(ctx, theme, state);
    }

    // ── JOIN GROUP MODAL ──
    if state.show_join_group_modal {
        draw_join_group_modal(ctx, theme, state);
    }

    // ── P2P GROUP background refresh (v0.303.0 — all off the UI thread) ──
    // First apply anything finished workers sent back, then schedule more.
    // Nothing here blocks: switching groups is instant and the periodic
    // refresh never freezes the UI (the prior synchronous ureq calls caused
    // the ~1s hang the operator hit).
    drain_p2p_loaders(state);
    {
        // Keep the left-rail group list fresh every ~6s while the chat page is
        // open, so membership changes on another client (incl. disband/leave)
        // propagate and the open group exits if it vanished.
        let list_due = state
            .p2p_groups_last_fetch
            .map(|t| t.elapsed().as_secs() >= 6)
            .unwrap_or(true);
        if list_due && state.p2p_groups_list_loader.is_none() {
            state.p2p_groups_last_fetch = Some(std::time::Instant::now());
            spawn_groups_list_refresh(state);
        }
    }
    if let Some(gid) = state.chat_active_channel.strip_prefix("p2pgroup:").map(|s| s.to_string()) {
        if state.p2p_group_active_id != gid {
            // Freshly switched to a group (e.g. via the URL/restore path) —
            // kick off a loading fetch. The click handler already does this,
            // but this covers any other way the active channel becomes a group.
            if state.p2p_group_loader.is_none() {
                spawn_group_load(state, &gid, true);
            }
        } else {
            // Periodic background reload of the open group (~2s) — picks up new
            // messages, rekeys, and roster changes without blocking. (Native has
            // no P2P push yet, so incoming arrives at this cadence; 2s keeps it
            // from feeling chunky. My own sends echo instantly + are preserved
            // across reloads, so a tighter poll can't blink them out.)
            let due = state
                .p2p_group_last_fetch
                .map(|t| t.elapsed().as_millis() >= 2000)
                .unwrap_or(true);
            if due && state.p2p_group_loader.is_none() {
                spawn_group_load(state, &gid, false);
            }
        }
        // Repaint so try_recv keeps draining even when the window is idle.
        ctx.request_repaint_after(std::time::Duration::from_millis(400));
    }
    // Even when no P2P group is open, a pending background loader (e.g. the
    // periodic list refresh) needs a near-future frame to be drained + applied.
    if state.p2p_group_loader.is_some() || state.p2p_groups_list_loader.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(400));
    }

    // ── HELP / COMMANDS MODAL ──
    if state.show_help_modal {
        draw_help_modal(ctx, theme, state);
    }

    // ── SEARCH MODAL ──
    if state.chat_search_open {
        draw_search_modal(ctx, theme, state);
    }

    // ── PINS MODAL ──
    if state.chat_pins_open {
        draw_pins_modal(ctx, theme, state);
    }
}

fn draw_pins_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
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

        ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
            for p in &pins {
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
    });
    if !open {
        state.chat_pins_open = false;
    }
}

fn draw_search_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
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

// ─────────────────────────────── LEFT PANEL ───────────────────────────────

fn draw_left_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let is_connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());

    // Show connect UI only when disconnected (no separate connection bar when connected)
    if !is_connected {
        Frame::NONE
            .fill(Color32::from_rgb(40, 30, 30))
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
                        // Full-PQ: manual Connect must advertise the Kyber key too.
                        state.ws_client = Some(crate::net::ws_client::WsClient::connect_with_kyber(
                            &ws_url, &name, &pubkey, &state.kyber_public_b64,
                        ));
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
        let bg = if is_active {
            Color32::from_rgb(45, 40, 55)
        } else if resp.hovered() {
            Color32::from_rgb(40, 38, 48)
        } else {
            Color32::from_rgb(32, 32, 38)
        };
        ui.painter().rect_filled(rect, 0.0, bg);

        if is_active {
            let bar = egui::Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
            ui.painter().rect_filled(bar, 0.0, Color32::from_rgb(160, 140, 200));
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
        &format!("DMs ({})", dm_count),
        collapsed,
        theme.dm_bg(),
        |ui| {
            let (cog_rect, cog_resp) = crate::gui::widgets::icons::icon_button(ui, 14.0);
            let cog_color = if cog_resp.hovered() { Color32::WHITE } else { Color32::from_rgb(160, 160, 170) };
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
                            if ui.button("Clear All DMs").clicked() {
                                state.chat_dms.clear();
                                state.dm_settings_popup_open = false;
                            }
                            if ui.button("DM Notifications").clicked() {
                                // TODO: toggle DM notifications
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

                    // Channel-row style DM entry
                    let row_height = theme.row_height;
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

                        // Active indicator bar
                        if is_active {
                            let bar = egui::Rect::from_min_size(full_rect.min, Vec2::new(3.0, full_rect.height()));
                            ui.painter().rect_filled(bar, 0.0, Color32::from_rgb(200, 80, 80));
                        }

                        let mut cursor_x = full_rect.left() + theme.item_padding + 2.0;
                        let cy = full_rect.center().y;

                        // Unread dot
                        if dm.unread {
                            let dot_r = theme.status_dot_size * 0.375;
                            ui.painter().circle_filled(egui::pos2(cursor_x + dot_r, cy), dot_r, Color32::from_rgb(200, 80, 80));
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
                    }

                    if response.clicked() && state.chat_active_channel != dm_channel {
                        state.chat_active_channel = dm_channel;
                        state.chat_messages.clear();
                        state.history_fetched = false;
                        // Request DM history from server
                        if let Some(ref client) = state.ws_client {
                            if client.is_connected() {
                                let msg = serde_json::json!({
                                    "type": "dm_open",
                                    "partner": dm.user_key,
                                });
                                client.send(&msg.to_string());
                            }
                        }
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
                        if ui.button(RichText::new("Close Conversation").color(Color32::from_rgb(200, 80, 80))).clicked() {
                            state.chat_dms.retain(|d| d.user_key != dm.user_key);
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

/// Synchronous (ureq) refresh of `state.p2p_groups` from the relay's
/// /api/v2/groups projection. Matches the existing `upload_image_png_blocking`
/// pattern — fine for occasional refreshes (create/join/first-render); promote
/// to a background tokio task if it ever feels janky.
pub(crate) fn refresh_p2p_groups(state: &mut GuiState) {
    let server_url = state.server_url.clone();
    let seed = match state.private_key_bytes.as_ref() {
        Some(s) if !s.is_empty() => s.clone(),
        _ => return,
    };
    let dilithium_hex = match crate::net::identity::derive_pq_identity(&seed) {
        Ok(id) => id.dilithium_hex,
        Err(e) => {
            log::warn!("refresh_p2p_groups: derive identity failed: {e}");
            return;
        }
    };
    match crate::net::api_v2::fetch_p2p_groups(&server_url, &dilithium_hex) {
        Ok(list) => {
            log::info!("refresh_p2p_groups: {} groups", list.len());
            state.p2p_groups = list;
        }
        Err(e) => {
            log::warn!("refresh_p2p_groups: fetch failed: {e}");
        }
    }
    state.p2p_groups_last_fetch = Some(std::time::Instant::now());
}

fn draw_groups_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // NOTE: first-render population is handled by the BACKGROUND list refresh at
    // the top of draw() (spawn_groups_list_refresh, which fires when
    // p2p_groups_last_fetch is None). We deliberately do NOT do a synchronous
    // fetch here — that blocked the UI thread on the first open of the Groups
    // section. The background path runs before this panel each frame, so the
    // list is requested without ever freezing the render loop.

    // Sort groups alphabetically by name (see draw_dm_section for rationale).
    state.chat_groups.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    state.p2p_groups.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    let collapsed = state.chat_groups_collapsed;
    let group_count = state.chat_groups.len() + state.p2p_groups.len();

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
        &format!("Groups ({})", group_count),
        collapsed,
        theme.group_bg(),
        |ui| {
            // + button (create group)
            {
                let (plus_rect, plus_resp) = crate::gui::widgets::icons::icon_button(ui, 14.0);
                let plus_color = if plus_resp.hovered() { Color32::WHITE } else { Color32::from_rgb(160, 160, 170) };
                crate::gui::widgets::icons::paint_plus(ui.painter(), plus_rect, plus_color);
                if plus_resp.on_hover_text("Create Group").clicked() {
                    create_clicked = true;
                }
            }
            // Join button (arrow icon)
            {
                let (arrow_rect, arrow_resp) = crate::gui::widgets::icons::icon_button(ui, 14.0);
                let arrow_color = if arrow_resp.hovered() { Color32::WHITE } else { Color32::from_rgb(160, 160, 170) };
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

                if state.chat_groups.is_empty() && state.p2p_groups.is_empty() {
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
                        let cog_color = if on_cog { theme.accent() } else { Color32::from_rgb(140, 140, 150) };
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
                            if ui.button("Leave group").clicked() {
                                p2p_leave_gid = Some(p.group_id.clone());
                            }
                            // Disband: creator only.
                            if p.is_creator && ui.button("Disband group (for everyone)").clicked() {
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

                // Groups render like servers (expandable header + nested channels)
                // but remain P2P under the hood. Header: <arrow> <cog> <name> <count>.
                // Channels inside route through `group_msg` with the group id. When
                // server-side multi-channel support lands, additional channels appear
                // here automatically from the GroupList data.
                let groups = state.chat_groups.clone();
                let ctx_time = ui.ctx().input(|i| i.time);
                let mut toggle_group_idx: Option<usize> = None;
                let mut leave_group_id: Option<String> = None;
                let mut open_server_settings = false;

                for (gi, group) in groups.iter().enumerate() {
                    if gi > 0 { ui.add_space(4.0); }

                    let hdr_height = 24.0;
                    let full_w = ui.available_width();
                    let (hdr_rect, _) = ui.allocate_exact_size(
                        Vec2::new(full_w, hdr_height),
                        egui::Sense::hover(),
                    );

                    let cog_click_rect;
                    if ui.is_rect_visible(hdr_rect) {
                        let hdr_bg = Color32::from_rgba_premultiplied(
                            theme.group_bg().r().saturating_add(20),
                            theme.group_bg().g().saturating_add(20),
                            theme.group_bg().b().saturating_add(20),
                            theme.group_bg().a(),
                        );
                        ui.painter().rect_filled(hdr_rect, 0.0, hdr_bg);

                        let mut cx = hdr_rect.left() + 8.0;
                        let cy = hdr_rect.center().y;

                        // Collapse arrow
                        let arrow_icon_rect = egui::Rect::from_min_size(egui::pos2(cx, cy - 5.0), Vec2::splat(10.0));
                        if group.collapsed {
                            crate::gui::widgets::icons::paint_triangle_right(ui.painter(), arrow_icon_rect, Color32::from_rgb(160, 160, 170));
                        } else {
                            crate::gui::widgets::icons::paint_triangle_down(ui.painter(), arrow_icon_rect, Color32::from_rgb(160, 160, 170));
                        }
                        cx += 14.0;

                        // Cog icon with RGB hover (matches nav)
                        cog_click_rect = egui::Rect::from_min_size(egui::pos2(cx - 2.0, hdr_rect.top()), Vec2::new(14.0, hdr_height));
                        let cog_icon_rect = egui::Rect::from_min_size(egui::pos2(cx, cy - 5.0), Vec2::splat(10.0));
                        let hover_pos = ui.ctx().input(|i| i.pointer.hover_pos().unwrap_or_default());
                        let on_cog = cog_click_rect.contains(hover_pos);
                        let cog_color = if on_cog { theme.accent() } else { Color32::from_rgb(140, 140, 150) };
                        crate::gui::widgets::icons::paint_cog(ui.painter(), cog_icon_rect, cog_color);
                        if on_cog {
                            let rgb = crate::gui::widgets::row::rgb_from_time(ctx_time);
                            ui.painter().rect_stroke(
                                cog_icon_rect.expand(2.0),
                                Rounding::same(3),
                                Stroke::new(1.0, rgb),
                                egui::StrokeKind::Outside,
                            );
                            ui.ctx().request_repaint();
                        }
                        cx += 14.0;

                        // Group name
                        ui.painter().text(
                            egui::pos2(cx, cy),
                            egui::Align2::LEFT_CENTER,
                            &group.name,
                            egui::FontId::proportional(theme.body_size),
                            theme.text_primary(),
                        );

                        // Member count on right
                        ui.painter().text(
                            egui::pos2(hdr_rect.right() - theme.item_padding, cy),
                            egui::Align2::RIGHT_CENTER,
                            &format!("{}", group.member_count),
                            egui::FontId::proportional(theme.small_size),
                            theme.text_muted(),
                        );
                    } else {
                        cog_click_rect = hdr_rect;
                    }

                    // Interact order: general header first, then specific arrow/cog so those
                    // win hit-test within their rects (egui: last interact wins).
                    let hdr_interact = ui.interact(hdr_rect, ui.id().with("grp_ctx").with(gi), egui::Sense::click());
                    let show_menu_rc = hdr_interact.secondary_clicked();

                    let arrow_click = egui::Rect::from_min_size(hdr_rect.min, Vec2::new(22.0, hdr_height));
                    let arrow_resp = ui.interact(arrow_click, ui.id().with("grp_arrow").with(gi), egui::Sense::click());
                    if arrow_resp.clicked() { toggle_group_idx = Some(gi); }
                    if arrow_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }

                    let cog_resp = ui.interact(cog_click_rect, ui.id().with("grp_cog").with(gi), egui::Sense::click());
                    if cog_resp.hovered() { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); }
                    let show_group_menu = cog_resp.clicked() || cog_resp.secondary_clicked();

                    let menu_id = ui.id().with("grp_menu").with(gi);
                    if show_group_menu || show_menu_rc {
                        ui.memory_mut(|m| m.toggle_popup(menu_id));
                    }
                    egui::popup_below_widget(ui, menu_id, &cog_resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
                        ui.set_min_width(180.0);
                        ui.label(RichText::new(&group.name).size(theme.font_size_body).color(theme.text_primary()).strong());
                        ui.separator();
                        if ui.button("Group Settings").clicked() {
                            open_server_settings = true;
                            state.chat_user_modal_key = group.id.clone(); // piggyback context
                        }
                        let invite_url = format!("https://united-humanity.us/chat/group/{}", group.id);
                        if ui.button("Copy Invite Link").clicked() { ui.ctx().copy_text(invite_url); }
                        if ui.button("Copy Group ID").clicked() { ui.ctx().copy_text(group.id.clone()); }
                        ui.separator();
                        if ui.button("What's a group vs a server?").clicked() {
                            state.active_help_topic = Some("groups-vs-servers".to_string());
                        }
                        ui.separator();
                        if ui.button(RichText::new("Leave Group").color(Color32::from_rgb(200, 80, 80))).clicked() {
                            leave_group_id = Some(group.id.clone());
                        }
                    });

                    // Channel rows (only when expanded). Same layout as server
                    // channels: voice-mic icon, settings cog, then the # name.
                    if !group.collapsed {
                        let is_group_admin = true; // TODO: per-group role once server reports it
                        for ch in group.channels.iter() {
                            let is_active = state.chat_active_channel == ch.id;
                            let accent = Color32::from_rgb(80, 200, 80);
                            let base_bg = if is_active {
                                Color32::from_rgb(accent.r() / 5 + 15, accent.g() / 5 + 15, accent.b() / 5 + 15)
                            } else {
                                theme.group_row_bg()
                            };
                            let row_w = ui.available_width();
                            let (row_rect, response) = ui.allocate_exact_size(
                                Vec2::new(row_w, theme.row_height),
                                egui::Sense::click(),
                            );

                            let mut voice_icon_rect = egui::Rect::NOTHING;
                            let mut gear_icon_rect = egui::Rect::NOTHING;

                            if ui.is_rect_visible(row_rect) {
                                let hover = response.hovered();
                                let fill = if hover && !is_active { theme.group_row_hover() } else { base_bg };
                                ui.painter().rect_filled(row_rect, 0.0, fill);
                                if is_active {
                                    let bar = egui::Rect::from_min_size(row_rect.min, Vec2::new(3.0, row_rect.height()));
                                    ui.painter().rect_filled(bar, 0.0, accent);
                                }

                                let text_color = if is_active { theme.text_primary() } else { theme.text_secondary() };
                                let icon_size = 12.0;
                                let cy = row_rect.center().y;
                                let mut cx = row_rect.left() + theme.item_padding + 2.0;
                                let hover_pos = ui.ctx().input(|i| i.pointer.hover_pos().unwrap_or_default());

                                // 1. Voice icon
                                if ch.voice_enabled {
                                    let icon_rect = egui::Rect::from_min_size(egui::pos2(cx, cy - icon_size * 0.5), Vec2::splat(icon_size));
                                    voice_icon_rect = egui::Rect::from_min_size(egui::pos2(cx - 2.0, row_rect.top()), Vec2::new(icon_size + 4.0, row_rect.height()));
                                    if ch.voice_joined {
                                        crate::gui::widgets::icons::paint_speaker(ui.painter(), icon_rect, theme.success());
                                    } else {
                                        let on_voice = response.hovered() && voice_icon_rect.contains(hover_pos);
                                        let mic_color = if on_voice { Color32::WHITE } else { Color32::from_rgb(100, 100, 110) };
                                        crate::gui::widgets::icons::paint_mic(ui.painter(), icon_rect, mic_color);
                                    }
                                    cx += icon_size + 2.0;
                                }

                                // (Per-channel cog removed in v0.187 — group
                                // channel admin lives in the group settings
                                // cog next to the group name.)

                                // 3. # channel name
                                let name_str = format!("# {}", ch.name);
                                ui.painter().text(
                                    egui::pos2(cx + 2.0, cy),
                                    egui::Align2::LEFT_CENTER,
                                    &name_str,
                                    egui::FontId::proportional(theme.body_size),
                                    text_color,
                                );
                                // Status icons (eye = read-only, node =
                                // federated). Same treatment as server
                                // channels for consistency. v0.244.
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
                                    // TODO: wire group voice join/leave through the relay
                                } else if gear_icon_rect.contains(click_pos) && is_group_admin {
                                    state.show_channel_edit_modal = true;
                                    state.edit_channel_id = ch.id.clone();
                                    state.edit_channel_name = ch.name.clone();
                                    state.edit_channel_description = ch.description.clone();
                                } else if state.chat_active_channel != ch.id {
                                    state.chat_active_channel = ch.id.clone();
                                    state.chat_messages.clear();
                                    state.history_fetched = false;
                                    if let Some(ref client) = state.ws_client {
                                        if client.is_connected() {
                                            let msg = serde_json::json!({
                                                "type": "group_history_request",
                                                "group_id": group.id,
                                            });
                                            client.send(&msg.to_string());
                                        }
                                    }
                                }
                            }
                            response.context_menu(|ui| {
                                let link = format!("https://united-humanity.us/chat/group/{}/{}", group.id, ch.name);
                                if ui.button("Copy Channel Link").clicked() {
                                    ui.ctx().copy_text(link);
                                    ui.close_menu();
                                }
                            });
                        }
                        // (The old "+ Channel (coming soon)" hint row
                        // was removed in v0.222 — channel creation is
                        // done via the group settings cog.)
                    }
                }

                if let Some(idx) = toggle_group_idx {
                    if let Some(g) = state.chat_groups.get_mut(idx) {
                        g.collapsed = !g.collapsed;
                    }
                }

                // Navigate to server-settings page if a group menu asked for it.
                // push_nav_to so Esc on ServerSettings returns to Chat
                // instead of jumping to FPS mode.
                if open_server_settings {
                    state.push_nav_to(crate::gui::GuiPage::ServerSettings);
                }

                // Apply group leave (send to server so groups don't come back)
                if let Some(gid) = leave_group_id {
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
                    if state.chat_active_channel.starts_with(&format!("group:{}", gid)) {
                        state.chat_active_channel = "general".to_string();
                    }
                }
            });
    }
}

// ── Servers Section ──

fn draw_servers_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // Sort servers alphabetically by name (see draw_dm_section for rationale).
    state.chat_servers.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    let collapsed = state.chat_servers_collapsed;

    // Build a virtual server from the current connection
    let connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());

    let virtual_server_count = if connected { 1 } else { 0 } + state.chat_servers.len();

    let header_label = format!("Servers ({})", virtual_server_count);

    let mut add_server_clicked = false;
    if tinted_section_header_with_buttons(
        ui,
        &header_label,
        collapsed,
        theme.server_bg(),
        |ui| {
            let (plus_rect, plus_resp) = crate::gui::widgets::icons::icon_button(ui, 14.0);
            let plus_color = if plus_resp.hovered() { Color32::WHITE } else { Color32::from_rgb(160, 160, 170) };
            crate::gui::widgets::icons::paint_plus(ui.painter(), plus_rect, plus_color);
            if plus_resp.on_hover_text("Add Server").clicked() {
                add_server_clicked = true;
            }
        },
    ) {
        state.chat_servers_collapsed = !state.chat_servers_collapsed;
        crate::config::AppConfig::from_gui_state(state).save();
    }
    // (disconnect is handled inside the server name row)
    if add_server_clicked {
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
                // Current connected server (virtual entry)
                if connected {
                    let online_count = state.chat_users.iter().filter(|u| u.status != "offline").count();
                    let svr_hdr_height = 24.0;
                    let svr_full_w = ui.available_width();
                    let svr_name = server_display_name(&state.server_url);
                    let svr_collapsed = state.chat_connected_server_collapsed;
                    let ctx_time = ui.ctx().input(|i| i.time);

                    // Server header: <collapse> <cog> <name (online)> ... <member count>
                    let (svr_rect, _) = ui.allocate_exact_size(Vec2::new(svr_full_w, svr_hdr_height), egui::Sense::hover());
                    let cog_click_rect;
                    let mut svr_disconnect = false;
                    if ui.is_rect_visible(svr_rect) {
                        let svr_bg = Color32::from_rgba_premultiplied(
                            theme.server_bg().r().saturating_add(20),
                            theme.server_bg().g().saturating_add(20),
                            theme.server_bg().b().saturating_add(20),
                            theme.server_bg().a(),
                        );
                        ui.painter().rect_filled(svr_rect, 0.0, svr_bg);

                        let mut cx = svr_rect.left() + 8.0;
                        let cy = svr_rect.center().y;

                        // Collapse arrow
                        let arrow_icon_rect = egui::Rect::from_min_size(egui::pos2(cx, cy - 5.0), Vec2::splat(10.0));
                        if svr_collapsed {
                            crate::gui::widgets::icons::paint_triangle_right(ui.painter(), arrow_icon_rect, Color32::from_rgb(160, 160, 170));
                        } else {
                            crate::gui::widgets::icons::paint_triangle_down(ui.painter(), arrow_icon_rect, Color32::from_rgb(160, 160, 170));
                        }
                        cx += 14.0;

                        // Cog icon (with RGB hover effect matching nav buttons)
                        cog_click_rect = egui::Rect::from_min_size(egui::pos2(cx - 2.0, svr_rect.top()), Vec2::new(14.0, svr_hdr_height));
                        let cog_icon_rect = egui::Rect::from_min_size(egui::pos2(cx, cy - 5.0), Vec2::splat(10.0));
                        let hover_pos = ui.ctx().input(|i| i.pointer.hover_pos().unwrap_or_default());
                        let on_cog = cog_click_rect.contains(hover_pos);
                        let cog_color = if on_cog { theme.accent() } else { Color32::from_rgb(140, 140, 150) };
                        crate::gui::widgets::icons::paint_cog(ui.painter(), cog_icon_rect, cog_color);
                        if on_cog {
                            let rgb = crate::gui::widgets::row::rgb_from_time(ctx_time);
                            ui.painter().rect_stroke(
                                cog_icon_rect.expand(2.0),
                                Rounding::same(3),
                                Stroke::new(1.0, rgb),
                                egui::StrokeKind::Outside,
                            );
                            ui.ctx().request_repaint();
                        }
                        cx += 14.0;

                        // Green dot
                        let dot_r = theme.status_dot_size / 2.0;
                        ui.painter().circle_filled(egui::pos2(cx + dot_r, cy), dot_r, theme.success());
                        cx += theme.status_dot_size + 4.0;

                        // Server name + online count
                        ui.painter().text(
                            egui::pos2(cx, cy),
                            egui::Align2::LEFT_CENTER,
                            &format!("{} ({})", svr_name, online_count),
                            egui::FontId::proportional(theme.body_size),
                            theme.text_primary(),
                        );

                        // Member count on right
                        ui.painter().text(
                            egui::pos2(svr_rect.right() - theme.item_padding, cy),
                            egui::Align2::RIGHT_CENTER,
                            &format!("{}", state.chat_users.len()),
                            egui::FontId::proportional(theme.small_size),
                            theme.text_muted(),
                        );
                    } else {
                        cog_click_rect = svr_rect;
                    }

                    // Full-header interact FIRST so later specific-region interacts (arrow, cog)
                    // take priority. Last-registered wins hit-test for overlapping rects.
                    let hdr_interact = ui.interact(svr_rect, ui.id().with("svr_ctx"), egui::Sense::click());
                    let show_menu_rc = hdr_interact.secondary_clicked();

                    // Collapse arrow click target (overrides hdr_interact in its 22px region)
                    let arrow_click = egui::Rect::from_min_size(svr_rect.min, Vec2::new(22.0, svr_hdr_height));
                    let arrow_resp = ui.interact(arrow_click, ui.id().with("svr_arrow"), egui::Sense::click());
                    if arrow_resp.clicked() {
                        state.chat_connected_server_collapsed = !state.chat_connected_server_collapsed;
                        crate::config::AppConfig::from_gui_state(state).save();
                    }
                    if arrow_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    // Cog click target - opens settings popup (registered last = priority)
                    let cog_resp = ui.interact(cog_click_rect, ui.id().with("svr_cog"), egui::Sense::click());
                    if cog_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    let show_svr_menu = cog_resp.clicked() || cog_resp.secondary_clicked();

                    let menu_id = ui.id().with("svr_menu");
                    if show_svr_menu || show_menu_rc {
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
                        if ui.button("Mute Server").clicked() {
                            // TODO: implement mute
                        }
                        if ui.button(RichText::new("Disconnect").color(Color32::from_rgb(200, 80, 80))).clicked() {
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
                                    let mic_color = if on_voice { Color32::WHITE } else { Color32::from_rgb(100, 100, 110) };
                                    crate::gui::widgets::icons::paint_mic(ui.painter(), icon_rect, mic_color);
                                }
                                cx += icon_size + 2.0;
                            }

                            // (Per-channel settings cog removed in v0.187 —
                            // channel admin lives in the Server Settings
                            // modal next to the server name. Click that
                            // single cog to manage every channel + role +
                            // member in one place.)

                            // 3. # Channel name
                            let name_str = format!("# {}", ch.name);
                            ui.painter().text(
                                egui::pos2(cx + 2.0, cy),
                                egui::Align2::LEFT_CENTER,
                                &name_str,
                                egui::FontId::proportional(theme.body_size),
                                text_color,
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
                            } else if state.chat_active_channel != ch.id {
                                // Only swap channel context when the click actually changes
                                // channels. Re-clicking the active channel used to clear
                                // chat_messages and re-fetch history, which nuked any
                                // local-echoed unsent reply (BUG-035). Now it's a no-op.
                                state.chat_active_channel = ch.id.clone();
                                state.chat_messages.clear();
                                state.history_fetched = false;
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

                    // Apply voice toggle after the loop
                    if let Some((idx, joining)) = voice_toggle_idx {
                        if let Some(ch) = state.chat_channels.get_mut(idx) {
                            let ch_name = ch.name.clone();
                            ch.voice_joined = joining;
                            let msg_type = if joining { "voice_join" } else { "voice_leave" };
                            log::info!("Voice {} requested: {}", msg_type, ch_name);
                            crate::debug::push_debug(format!("Voice: {} for channel '{}'", msg_type, ch_name));
                            if let Some(ref client) = state.ws_client {
                                if client.is_connected() {
                                    let msg = serde_json::json!({
                                        "type": msg_type,
                                        "channel": ch_name,
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

                    ui.add_space(2.0);
                    } // end if !svr_collapsed
                }

                // Additional servers from chat_servers list
                for server in state.chat_servers.clone().iter() {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(&server.name)
                                .size(theme.font_size_body)
                                .color(theme.text_primary())
                                .strong(),
                        );
                    });
                    for ch in &server.channels {
                        ui.horizontal(|ui| {
                            ui.add_space(20.0);
                            ui.label(
                                RichText::new(format!("# {}", ch.name))
                                    .size(theme.font_size_body)
                                    .color(theme.text_secondary()),
                            );
                        });
                    }
                }
            });
    }

    ui.add_space(2.0);
}

// ─────────────────────────────── RIGHT PANEL ──────────────────────────────

fn draw_right_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ScrollArea::vertical()
        .id_salt("chat_right_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // ── Studio Section (broadcast / livestream quick-access) ──
            // Mirrors the website's studio widget placement at the top of the
            // right rail. Keeps Go Live + the full Studio page reachable even
            // after the main-menu consolidation folds away the top-nav button.
            draw_studio_section(ui, theme, state);

            ui.add_space(4.0);

            // ── Friends Section ──
            draw_friends_section(ui, theme, state);

            ui.add_space(4.0);

            // ── Server Members Section ──
            draw_members_section(ui, theme, state);
        });
}

/// Studio (broadcast/livestream) quick-access section at the very top of the
/// chat right rail — a native mirror of the website's studio widget. The point
/// is durability: when the top nav condenses into Real/Play pages, the current
/// nav-bar Studio button disappears, so streamers reach Go Live and the full
/// Studio page from here instead. Collapse state persists via AppConfig.
fn draw_studio_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let collapsed = state.chat_studio_collapsed;
    let live = state.studio.is_live;

    // Header carries a LIVE badge at a glance (em-dash + text — both render
    // reliably in the bundled font; no risky glyphs).
    let title = if live { "Studio, LIVE" } else { "Studio" };
    if section_header(ui, title, collapsed, theme.bg_tertiary()) {
        state.chat_studio_collapsed = !state.chat_studio_collapsed;
        crate::config::AppConfig::from_gui_state(state).save();
    }

    if collapsed {
        return;
    }

    ui.spacing_mut().item_spacing.y = theme.row_gap;
    ui.add_space(4.0);

    // One-line status / purpose caption.
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        let (caption, color) = if live {
            ("Broadcasting now", theme.success())
        } else {
            ("Broadcast & livestream", theme.text_muted())
        };
        ui.label(
            RichText::new(caption)
                .size(theme.font_size_small)
                .color(color),
        );
    });

    // Action row: Go Live / End Stream toggle + Open Studio (full page).
    ui.horizontal(|ui| {
        ui.add_space(12.0);
        ui.spacing_mut().item_spacing.x = 6.0;

        if live {
            if widgets::Button::danger("End Stream").show(ui, theme) {
                state.studio.is_live = false;
                state.studio.is_paused = false;
            }
        } else if widgets::Button::primary("Go Live").show(ui, theme) {
            state.studio.is_live = true;
            state.studio.is_paused = false;
            state.studio.live_start_time = ui.ctx().input(|i| i.time);
        }

        // push_nav_to so Esc on the Studio page returns to Chat.
        if widgets::Button::secondary("Open Studio").show(ui, theme) {
            state.push_nav_to(crate::gui::GuiPage::Studio);
        }
    });
    ui.add_space(4.0);
}

/// Shared user row renderer for both friends and members lists.
/// Ensures consistent spacing, hover, click handling, and layout.
fn draw_user_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    name: &str,
    public_key: &str,
    role: &str,
    status: &str,
    state: &mut GuiState,
    ctx_time: f64,
) {
    let is_modal_target = state.chat_user_modal_open
        && state.chat_user_modal_key == public_key;

    let row_height = theme.row_height;
    let (full_rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), row_height),
        egui::Sense::click(),
    );

    if ui.is_rect_visible(full_rect) {
        let bg = if response.hovered() {
            Color32::from_rgb(45, 45, 55)
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(full_rect, 0.0, bg);

        // RGB border when this user's modal is open
        if is_modal_target {
            let border_color = crate::gui::widgets::row::rgb_from_time(ctx_time);
            ui.painter().rect_stroke(
                full_rect, 2.0,
                egui::Stroke::new(1.5, border_color),
                egui::epaint::StrokeKind::Inside,
            );
            ui.ctx().request_repaint();
        }

        let mut cx = full_rect.left() + theme.item_padding;
        let cy = full_rect.center().y;

        // Online/offline dot
        let dot_color = match status {
            "offline" => Color32::from_rgb(100, 100, 100),
            "away" => theme.warning(),
            "busy" | "dnd" => theme.danger(),
            _ => theme.success(),
        };
        let dot_r = theme.status_dot_size / 2.0;
        ui.painter().circle_filled(egui::pos2(cx + dot_r, cy), dot_r, dot_color);
        cx += theme.status_dot_size + 4.0;

        // Name
        let nc = if status == "offline" { theme.text_muted() } else { theme.text_primary() };
        let name_galley = ui.painter().layout_no_wrap(
            name.to_string(),
            egui::FontId::proportional(theme.body_size),
            nc,
        );
        ui.painter().galley(egui::pos2(cx, cy - name_galley.size().y / 2.0), name_galley, nc);
        // Advance past name for role badges
        let name_w = ui.painter().layout_no_wrap(
            name.to_string(),
            egui::FontId::proportional(theme.body_size),
            nc,
        ).size().x;
        cx += name_w + 4.0;

        // Role badge (single letter pill)
        if !role.is_empty() && role != "member" {
            let (badge_color, badge_letter) = match role {
                "admin" => (Color32::from_rgb(231, 76, 60), "A"),
                "mod" | "moderator" => (Color32::from_rgb(155, 89, 182), "M"),
                "verified" => (Color32::from_rgb(52, 152, 219), "V"),
                "donor" => (Color32::from_rgb(241, 196, 15), "D"),
                _ => (Color32::from_rgb(100, 100, 100), "?"),
            };
            let badge_rect = egui::Rect::from_min_size(
                egui::pos2(cx, cy - 7.0),
                Vec2::new(14.0, 14.0),
            );
            ui.painter().rect_filled(badge_rect, Rounding::same(3), badge_color);
            ui.painter().text(
                badge_rect.center(),
                egui::Align2::CENTER_CENTER,
                badge_letter,
                egui::FontId::proportional(9.0),
                Color32::WHITE,
            );
        }
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if response.clicked() {
        state.chat_user_modal_open = true;
        state.chat_user_modal_name = name.to_string();
        state.chat_user_modal_key = public_key.to_string();
    }
}

fn draw_friends_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let collapsed = state.chat_friends_collapsed;
    let friend_count = state.chat_friends.len();

    if section_header(ui, &format!("Friends ({})", friend_count), collapsed, Color32::from_rgb(35, 35, 42)) {
        state.chat_friends_collapsed = !state.chat_friends_collapsed;
        crate::config::AppConfig::from_gui_state(state).save();
    }

    if !collapsed {
        ui.spacing_mut().item_spacing.y = theme.row_gap;

        if state.chat_friends.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(
                    RichText::new("No friends added yet")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            });
            ui.add_space(4.0);
        }

        let ctx_time = ui.ctx().input(|i| i.time);
        let friends = state.chat_friends.clone();
        for friend in &friends {
            draw_user_row(ui, theme, &friend.name, &friend.public_key, &friend.role, &friend.status, state, ctx_time);
        }
    }
}

fn draw_members_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let collapsed = state.chat_members_collapsed;
    let member_count = state.chat_users.len();
    let server_name = server_display_name(&state.server_url);

    if section_header(ui, &format!("{} ({})", server_name, member_count), collapsed, Color32::from_rgb(35, 35, 42)) {
        state.chat_members_collapsed = !state.chat_members_collapsed;
        crate::config::AppConfig::from_gui_state(state).save();
    }

    if !collapsed {
        ui.spacing_mut().item_spacing.y = theme.row_gap;

        if state.chat_users.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(
                    RichText::new("No users online")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            });
            ui.add_space(4.0);
        }

        // Sort: online first, then alphabetical
        let mut users = state.chat_users.clone();
        users.sort_by(|a, b| {
            let a_online = a.status != "offline";
            let b_online = b.status != "offline";
            b_online.cmp(&a_online).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        let ctx_time = ui.ctx().input(|i| i.time);
        for user in &users {
            draw_user_row(ui, theme, &user.name, &user.public_key, &user.role, &user.status, state, ctx_time);
        }

        // ── DEV: native WebRTC P2P transport self-test (increment 1) ──
        // Minimal proof button: pick a target peer (the user whose profile
        // modal is open, else the first online user that isn't us) and open a
        // DataChannel to them. On open, lib.rs auto-sends "native p2p test";
        // received frames + open/close show in the in-app Debug console. This
        // is a transport proof, not polished UI — it lives only in native
        // builds and only when a WebRTC manager is up.
        #[cfg(feature = "native")]
        {
            // Resolve a target: prefer the modal-open user, else first online
            // non-self user from the sorted list.
            let me = state.profile_public_key.clone();
            let target: Option<(String, String)> = {
                let modal_key = if state.chat_user_modal_open {
                    Some(state.chat_user_modal_key.clone())
                } else {
                    None
                };
                modal_key
                    .filter(|k| !k.is_empty() && *k != me)
                    .and_then(|k| {
                        users.iter().find(|u| u.public_key == k)
                            .map(|u| (u.public_key.clone(), u.name.clone()))
                    })
                    .or_else(|| {
                        users.iter()
                            .find(|u| u.status != "offline" && u.public_key != me && !u.public_key.is_empty())
                            .map(|u| (u.public_key.clone(), u.name.clone()))
                    })
            };

            ui.add_space(6.0);
            if let Some((peer_key, peer_name)) = target {
                let label = format!("P2P test \u{2192} {}", peer_name);
                if widgets::Button::secondary(&label).full_width().show(ui, theme) {
                    if let Some(ref webrtc) = state.webrtc {
                        // Arm the one-shot test send for when the channel opens.
                        state.webrtc_test_peer = Some(peer_key.clone());
                        // Offer (honors the offerer rule internally: only the
                        // larger pubkey actually offers; the smaller side waits
                        // for the peer's offer — both presses on both machines
                        // are harmless).
                        webrtc.offer_to(peer_key.clone());
                        crate::debug::push_debug(format!(
                            "WebRTC: P2P test initiated to {}",
                            if peer_key.len() > 12 { &peer_key[..12] } else { &peer_key }
                        ));
                    } else {
                        crate::debug::push_debug("WebRTC: manager not ready (connect to a server first)");
                    }
                }
            } else {
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("P2P test: no online peer")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                });
            }
        }
    }
}

// ─────────────────────────────── CENTER PANEL ─────────────────────────────

fn draw_center_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // ── Channel header ──
    // Lock buttons moved OUT of the header in v0.189.x — operator wanted
    // them tucked into the actual panel CORNERS, not next to the channel
    // title where they could be mistaken for a UI label. They now paint
    // as floating Areas anchored to the side-panel boundaries (see the
    // overlays at the bottom of `chat::draw`).
    Frame::NONE
        .fill(Color32::from_rgb(25, 25, 30))
        .inner_margin(egui::Margin::symmetric(16, 10))
        .stroke(Stroke::new(1.0, Color32::from_rgb(40, 40, 48)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Force the horizontal layout (and therefore the Frame
                // around it) to fill the full available panel width.
                // Without this, ui.horizontal shrinks to its content's
                // bounding rect, so the dark-gray header background only
                // covered the left portion next to the channel name and
                // left a black void on the right when the description
                // was short. The header should always read as one
                // continuous bar across the panel.
                ui.set_min_width(ui.available_width());
                let ac = state.chat_active_channel.clone();
                if ac.starts_with("dm:") {
                    // DM header: back button + partner name
                    if widgets::Button::ghost("\u{2190} Back").show(ui, theme) {
                        state.chat_active_channel = "general".to_string();
                    }
                    let partner_key = &ac[3..];
                    let partner_name = state.chat_dms.iter()
                        .find(|d| d.user_key == partner_key)
                        .map(|d| d.user_name.clone())
                        .unwrap_or_else(|| partner_key.to_string());
                    ui.label(
                        RichText::new(format!("DM: {}", partner_name))
                            .size(theme.font_size_heading)
                            .color(Color32::from_rgb(220, 120, 120))
                            .strong(),
                    );
                } else if let Some(gid) = ac.strip_prefix("p2pgroup:") {
                    // P2P group header: back button + name + a Copy-invite
                    // action. Leave / Disband live in the left-rail cog/
                    // right-click menu (kept out of this row so it can't
                    // overflow + clip on a narrow panel — that's why the
                    // operator couldn't reach them before).
                    if widgets::Button::ghost("\u{2190} Back").show(ui, theme) {
                        state.chat_active_channel = "general".to_string();
                    }
                    let gid = gid.to_string();
                    let group_name = state.p2p_groups.iter()
                        .find(|g| g.group_id == gid)
                        .map(|g| g.name.clone())
                        .unwrap_or_else(|| gid.clone());
                    ui.label(
                        RichText::new(&group_name)
                            .size(theme.font_size_heading)
                            .color(Color32::from_rgb(120, 220, 120))
                            .strong(),
                    );
                    // E2EE signal via plain text — the egui font has no lock
                    // emoji glyph (it tofus), so we say it in words, not a 🔒.
                    ui.label(
                        RichText::new("end-to-end encrypted")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                    if widgets::Button::ghost("Copy invite").show(ui, theme) {
                        mint_and_copy_p2p_invite(ui.ctx(), state, &gid, &group_name);
                    }
                    if !state.p2p_group_invite_status.is_empty() {
                        ui.label(
                            RichText::new(format!("  {}", state.p2p_group_invite_status))
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                    }
                } else if ac.starts_with("group:") {
                    // Group header: back button + group name
                    if widgets::Button::ghost("\u{2190} Back").show(ui, theme) {
                        state.chat_active_channel = "general".to_string();
                    }
                    let group_id = &ac[6..];
                    let group_name = state.chat_groups.iter()
                        .find(|g| g.id == group_id)
                        .map(|g| g.name.clone())
                        .unwrap_or_else(|| group_id.to_string());
                    ui.label(
                        RichText::new(format!("Group: {}", group_name))
                            .size(theme.font_size_heading)
                            .color(Color32::from_rgb(120, 220, 120))
                            .strong(),
                    );
                } else {
                    // Normal channel header
                    ui.label(
                        RichText::new(format!("# {}", state.chat_active_channel))
                            .size(theme.font_size_heading)
                            .color(theme.text_primary())
                            .strong(),
                    );

                    let desc = state
                        .chat_channels
                        .iter()
                        .find(|c| c.id == state.chat_active_channel)
                        .map(|c| c.description.as_str())
                        .unwrap_or("");
                    if !desc.is_empty() {
                        ui.label(
                            RichText::new(format!("  |  {}", desc))
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                    }
                }
            });
        });

    // ── Message area ──
    let active_channel = state.chat_active_channel.clone();
    // Input height grows by 22 px when a reply context is active so the
    // "Replying to … [X]" banner has its own row above the text field.
    let reply_banner_h: f32 = if state.chat_reply_to.is_some() { 22.0 } else { 0.0 };
    // The "… is typing" indicator renders as its OWN row above the text field
    // too, so reserve its height the same way. Without this, an incoming typing
    // event grew the input bar's CONTENT but not its allocated rect, sliding the
    // composer down past the window's bottom edge — and behind the taskbar in
    // snapped-fullscreen (the operator's bug report). Mirror the freshness check
    // the renderer uses below (entries within TYPING_TTL = 3 s) so the reserved
    // height matches whether the row is actually drawn this frame.
    let typing_active = {
        let now = std::time::Instant::now();
        state
            .chat_typing_users
            .values()
            .any(|(_, t)| now.duration_since(*t) < std::time::Duration::from_secs(3))
    };
    let typing_h: f32 = if typing_active { 22.0 } else { 0.0 };
    let input_height = 52.0 + reply_banner_h + typing_h;

    let available = ui.available_rect_before_wrap();
    let messages_rect = egui::Rect::from_min_size(
        available.min,
        Vec2::new(available.width(), available.height() - input_height),
    );

    ui.allocate_ui_at_rect(messages_rect, |ui| {
        ScrollArea::vertical()
            .id_salt("chat_messages_scroll")
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(8.0);

                let filtered: Vec<&ChatMessage> = state
                    .chat_messages
                    .iter()
                    .filter(|m| m.channel == active_channel)
                    .collect();

                if filtered.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        if active_channel.starts_with("p2pgroup:") {
                            // P2P group: distinguish "no key yet" from "no
                            // messages yet" so the user knows what to do.
                            let gid = &active_channel["p2pgroup:".len()..];
                            let gname = state.p2p_groups.iter()
                                .find(|g| g.group_id == gid)
                                .map(|g| g.name.clone())
                                .unwrap_or_else(|| gid.to_string());
                            ui.label(
                                RichText::new(&gname)
                                    .size(theme.font_size_title)
                                    .color(theme.text_primary()),
                            );
                            ui.add_space(8.0);
                            let hint = if state.p2p_group_loading {
                                "Loading…"
                            } else if state.p2p_group_chat_epoch_key.is_none() {
                                "No epoch key yet. The group creator must open this group once to issue the first key, after that, everyone with an invite can read and write."
                            } else {
                                "No messages yet. Your messages here are end-to-end encrypted under the group key."
                            };
                            ui.label(
                                RichText::new(hint)
                                    .size(theme.font_size_body)
                                    .color(theme.text_muted()),
                            );
                        } else {
                            ui.label(
                                RichText::new(format!("Welcome to #{}", active_channel))
                                    .size(theme.font_size_title)
                                    .color(theme.text_primary()),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("No messages yet. Say something!")
                                    .size(theme.font_size_body)
                                    .color(theme.text_muted()),
                            );
                        }
                    });
                }

                // Track alternating user colors
                let mut last_sender = String::new();
                let mut sender_parity = false; // toggles each time sender changes
                let bg_even = Color32::from_rgb(8, 8, 10);
                let bg_odd = Color32::from_rgb(16, 16, 20);
                let ctx_time = ui.ctx().input(|i| i.time);

                // Reactions clicked during render — applied after the loop ends
                // so we don't try to send WS messages while iterating &state.
                let mut pending_reactions: Vec<(String, u64, String)> = Vec::new();
                // Reply button clicks: defer setting state.chat_reply_to until after the loop.
                let mut pending_reply: Option<crate::gui::ReplyContext> = None;
                // Pin button clicks — defer pin_request WS sends.
                let mut pending_pins: Vec<(String, String, String, u64)> = Vec::new();
                // Edit button clicks — defer setting edit target.
                let mut pending_edit: Option<(u64, String)> = None;
                // Edit-save submissions from the inline editor.
                let mut pending_edit_save: Option<(u64, String)> = None;
                // Cancel flag — Cancel button in the inline editor sets this
                // so we clear chat_edit_target after the loop ends.
                let mut pending_edit_cancel = false;
                // Report button clicks (from context menu) — buffered to send /report slash command.
                let mut pending_reports: Vec<(String, String)> = Vec::new();
                // v0.281.0: Delete button clicks (from context menu) — buffered
                // so the borrow on `state.chat_messages` ends before we mutate
                // state to send WS. Pairs as (sender_key, timestamp_ms); the
                // server fills the rest from its own auth context.
                let mut pending_deletes: Vec<(String, u64)> = Vec::new();

                // Remove default item spacing so rows sit flush
                ui.spacing_mut().item_spacing = Vec2::ZERO;

                // Deferred avatars — collected during the loop, painted
                // AFTER all rows are drawn so 32×32 avatars on short
                // single-line header rows don't get clipped by the next
                // row's bg fill. Eliminates the empty-row gap below
                // short messages (operator feedback 2026-05-12).
                let mut deferred_avatars: Vec<crate::gui::widgets::row::DeferredAvatar> = Vec::new();

                for msg in &filtered {
                    let show_header = msg.sender_name != last_sender;
                    if show_header {
                        sender_parity = !sender_parity;
                    }
                    last_sender = msg.sender_name.clone();

                    let row_bg = if sender_parity { bg_even } else { bg_odd };
                    let icon_color = name_color(&msg.sender_name);
                    let icon_letter = msg.sender_name.chars().next().unwrap_or('?');
                    let channeling = state.chat_user_modal_open
                        && msg.sender_key == state.chat_user_modal_key;
                    // Extract image URLs from the message so we can render them
                    // as inline thumbnails instead of raw /uploads/... text.
                    let image_urls = crate::gui::widgets::image_cache::extract_image_urls(&msg.content);
                    let display_text = if image_urls.is_empty() {
                        msg.content.clone()
                    } else {
                        crate::gui::widgets::image_cache::strip_image_urls(&msg.content)
                    };

                    // ── Reply-to context (if this is a reply) ──
                    if let Some(ref reply) = msg.reply_to {
                        let row_w = ui.available_width();
                        let (row_rect, _) = ui.allocate_exact_size(
                            Vec2::new(row_w, 18.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(row_rect, 0.0, row_bg);
                        let preview = if reply.preview.len() > 60 {
                            format!("{}…", &reply.preview[..60])
                        } else {
                            reply.preview.clone()
                        };
                        ui.painter().text(
                            egui::pos2(row_rect.left() + 40.0, row_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            &format!("↩ {}: {}", reply.sender_name, preview),
                            egui::FontId::proportional(theme.font_size_small),
                            theme.text_muted(),
                        );
                    }

                    // Inline editor for this message (only on user's own messages
                    // matching chat_edit_target). Otherwise render the regular row.
                    let is_editing = state.chat_edit_target
                        .as_ref()
                        .map(|(ts, _)| *ts == msg.timestamp_ms && msg.sender_key == state.profile_public_key)
                        .unwrap_or(false);

                    let need_text_row = show_header || !display_text.trim().is_empty();
                    // Track row hover + row rect + pill rect so the expanded
                    // pill popup (rendered later) can anchor to the right
                    // place. The popup is an egui::Area, doesn't allocate
                    // layout space — hovering a message no longer shifts
                    // every message below.
                    let mut row_was_hovered = false;
                    let mut row_rect_opt: Option<egui::Rect> = None;
                    let mut pill_rect_for_msg: egui::Rect = egui::Rect::NOTHING;
                    // Þ icon rect (within the pill); reaction popup hover
                    // is now gated on the cursor being over THIS rect or
                    // over the popup itself, NOT the full pill (operator
                    // feedback 2026-05-12 — hovering message text should
                    // not open the popup so the user can copy/select text).
                    let mut thorn_rect_for_msg: egui::Rect = egui::Rect::NOTHING;
                    if is_editing {
                        // Render an editable row in place of the message text.
                        if let Some((_, ref mut draft)) = state.chat_edit_target {
                            ui.horizontal(|ui| {
                                ui.add_space(40.0);
                                let resp = ui.add(
                                    egui::TextEdit::multiline(draft)
                                        .desired_width(ui.available_width() - 130.0)
                                        .desired_rows(2),
                                );
                                ui.add_space(theme.spacing_xs);
                                if widgets::Button::primary("Save").show(ui, theme)
                                    || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                                {
                                    pending_edit_save = Some((msg.timestamp_ms, draft.trim().to_string()));
                                }
                                if widgets::Button::secondary("Cancel").show(ui, theme) {
                                    pending_edit_cancel = true;
                                }
                            });
                        }
                    } else if need_text_row {
                        // Measure exact pill width — must match what
                        // paint_timestamp_pill draws or the reserved space
                        // in message_row will be wrong and content text
                        // overlaps the pill (operator-reported bug).
                        let pill_width = compute_pill_width(ui.ctx(), theme, &msg.timestamp, &msg.reactions);

                        // Parse @mentions that resolve to a known user, for
                        // accent highlighting + click-to-open-modal (Discord-
                        // style). Longest-match against the user list so
                        // multi-word names like "@Deploy Bot" work. Ranges
                        // are char-indexed into display_text. Operator
                        // request 2026-05-15.
                        let mut mention_ranges: Vec<(usize, usize)> = Vec::new();
                        let mut mention_targets: Vec<(String, String)> = Vec::new();
                        {
                            let chars: Vec<char> = display_text.chars().collect();
                            let mut i = 0;
                            while i < chars.len() {
                                if chars[i] == '@' {
                                    let after: String = chars[i + 1..].iter().collect();
                                    let after_lower = after.to_lowercase();
                                    let mut best: Option<&crate::gui::ChatUser> = None;
                                    let mut best_len = 0usize;
                                    for u in &state.chat_users {
                                        if u.name.is_empty() { continue; }
                                        let nl = u.name.to_lowercase();
                                        if after_lower.starts_with(&nl) {
                                            let nlen = u.name.chars().count();
                                            // Boundary: char after the name
                                            // must be missing or non-word so
                                            // "@Eve" doesn't match user "Ev".
                                            let boundary_ok = after
                                                .chars()
                                                .nth(nlen)
                                                .map(|c| !c.is_alphanumeric())
                                                .unwrap_or(true);
                                            if boundary_ok && nlen > best_len {
                                                best = Some(u);
                                                best_len = nlen;
                                            }
                                        }
                                    }
                                    if let Some(u) = best {
                                        mention_ranges.push((i, 1 + best_len));
                                        mention_targets.push((u.name.clone(), u.public_key.clone()));
                                        i += 1 + best_len;
                                        continue;
                                    }
                                }
                                i += 1;
                            }
                        }

                        let row_resp = crate::gui::widgets::row::message_row(
                            ui,
                            theme,
                            icon_letter,
                            icon_color,
                            &msg.sender_name,
                            &msg.timestamp,
                            &display_text,
                            show_header,
                            row_bg,
                            channeling,
                            ctx_time,
                            pill_width,
                            &mention_ranges,
                        );
                        // Click on a highlighted @mention → open that user's
                        // modal (same as clicking them in the user list).
                        if let Some(idx) = row_resp.clicked_mention {
                            if let Some((nm, key)) = mention_targets.get(idx) {
                                state.chat_user_modal_open = true;
                                state.chat_user_modal_name = nm.clone();
                                state.chat_user_modal_key = key.clone();
                            }
                        }
                        row_was_hovered = row_resp.response.hovered();
                        row_rect_opt = Some(row_resp.response.rect);
                        pill_rect_for_msg = row_resp.pill_rect;
                        if let Some(ref a) = row_resp.deferred_avatar {
                            deferred_avatars.push(a.clone());
                        }

                        // Paint the timestamp pill into the rect message_row reserved.
                        // Returns the Þ icon's rect — used below to constrain
                        // the reaction-popup hover area so the popup doesn't
                        // open when the user is just trying to read/copy the
                        // message text (operator feedback 2026-05-12).
                        // `popup_active` switches the Þ from static accent to
                        // the channeling RGB cycle while the popup is open
                        // for this message (visual feedback that the Þ is
                        // "live"; operator feedback 2026-05-12).
                        let popup_active_for_this = state.chat_open_popup_ts == Some(msg.timestamp_ms);
                        if pill_rect_for_msg != egui::Rect::NOTHING {
                            thorn_rect_for_msg = paint_timestamp_pill(
                                ui,
                                theme,
                                pill_rect_for_msg,
                                &msg.timestamp,
                                &msg.reactions,
                                &state.profile_public_key,
                                msg.timestamp_ms,
                                msg.sender_key.clone(),
                                &mut pending_reactions,
                                popup_active_for_this,
                            );
                        }
                        if row_resp.userbox_clicked(ui.ctx()) {
                            state.chat_user_modal_open = true;
                            state.chat_user_modal_name = msg.sender_name.clone();
                            state.chat_user_modal_key = msg.sender_key.clone();
                        }

                        // Right-click context menu — same actions as the inline
                        // pill buttons but quicker to reach.
                        let is_own = msg.sender_key == state.profile_public_key;
                        // v0.281.0: derive my role from the populated user list.
                        // chat_users is the authoritative client-side mirror of
                        // server-known roles (driven by relay's user_list /
                        // peer_joined events); looking up my own pubkey there
                        // gives the server-visible role without a roundtrip.
                        // Falls back to "" when not yet populated (pre-list
                        // frames) — the Delete entry just won't render those
                        // frames; harmless.
                        let my_role = state.chat_users.iter()
                            .find(|u| u.public_key == state.profile_public_key)
                            .map(|u| u.role.clone())
                            .unwrap_or_default();
                        let is_admin_or_mod = my_role == "admin" || my_role == "mod" || my_role == "moderator";
                        row_resp.response.context_menu(|ui| {
                            ui.set_min_width(160.0);
                            // Plain text labels — leading emoji glyphs were
                            // unreliable across the loaded font (📋 📌 ✎ all
                            // rendered as tofu in some sessions). The context
                            // menu prioritizes legibility over ornament.
                            if ui.button("Copy text").clicked() {
                                ui.ctx().copy_text(msg.content.clone());
                                ui.close_menu();
                            }
                            // ↩ U+21A9 is in the Arrows block which IS in the
                            // loaded font — safe to keep.
                            if msg.timestamp_ms > 0 && ui.button("↩ Quote / reply").clicked() {
                                let preview = if msg.content.len() > 80 {
                                    format!("{}…", &msg.content[..80])
                                } else {
                                    msg.content.clone()
                                };
                                pending_reply = Some(crate::gui::ReplyContext {
                                    sender_key: msg.sender_key.clone(),
                                    sender_name: msg.sender_name.clone(),
                                    preview,
                                    timestamp_ms: msg.timestamp_ms,
                                });
                                ui.close_menu();
                            }
                            if msg.timestamp_ms > 0 && ui.button("Pin message").clicked() {
                                pending_pins.push((
                                    msg.sender_key.clone(),
                                    msg.sender_name.clone(),
                                    msg.content.clone(),
                                    msg.timestamp_ms,
                                ));
                                ui.close_menu();
                            }
                            if is_own && msg.timestamp_ms > 0 && ui.button("Edit").clicked() {
                                pending_edit = Some((msg.timestamp_ms, msg.content.clone()));
                                ui.close_menu();
                            }
                            // v0.281.0: Delete entry — own messages always; any
                            // message when the current user is admin or mod.
                            // Server enforces the same predicate so a stale UI
                            // can't bypass; we just hide the option when it's
                            // certain to be rejected.
                            if msg.timestamp_ms > 0 && (is_own || is_admin_or_mod) {
                                let label = if is_own { "Delete" } else { "Delete (admin)" };
                                if ui.button(label).clicked() {
                                    pending_deletes.push((
                                        msg.sender_key.clone(),
                                        msg.timestamp_ms,
                                    ));
                                    ui.close_menu();
                                }
                            }
                            ui.separator();
                            if ui.button("Report").clicked() {
                                pending_reports.push((msg.sender_name.clone(), msg.content.clone()));
                                ui.close_menu();
                            }
                        });
                    }

                    // (Old inline reaction PILLS row removed — reactions
                    // now live INSIDE the timestamp pill itself. See
                    // paint_timestamp_pill() above for the inline display.)
                    let target_ts = msg.timestamp_ms;
                    let target_from = msg.sender_key.clone();

                    // ── Expanded pill popup (right of Þ, sticky hover) ──
                    // Opens when the cursor is over the pill OR over the popup
                    // itself (sticky combined region), so moving from pill to
                    // popup doesn't dismiss it. Extends RIGHTWARD from the
                    // pill so it reads as the pill "growing" with new
                    // controls — functions on the LEFT (separated from the
                    // existing Þ that's already in the inline pill), reactions
                    // on the RIGHT, ∞ at the far right.
                    if pill_rect_for_msg != egui::Rect::NOTHING && target_ts > 0 {
                        // Reactions list. As of v0.190.0 we install the OS's
                        // emoji font (Windows seguiemj.ttf, macOS Apple Color
                        // Emoji, Linux Noto Color Emoji) as an egui font
                        // fallback at startup (see src/gui/fonts.rs). That
                        // covers basically every emoji in BMP + supplementary
                        // plane, so we can use a real reaction palette here.
                        // The icon_glyph_lint test still catches U+FE0F
                        // variation selectors and known-broken glyphs.
                        const TOP_REACTIONS: &[&str] = &[
                            "❤", "👍", "👎", "😂", "🤣", "😢", "😡", "🔥", "💯", "⭐",
                        ];
                        const ALL_REACTIONS: &[&str] = &[
                            // Hearts (colored) — system font supplies these.
                            "❤", "🧡", "💛", "💚", "💙", "💜", "🤍", "🖤", "🤎",
                            // Faces — laughs, cries, surprises, anger, love.
                            "😂", "🤣", "😢", "😭", "😡", "🤬", "😮", "😱", "🥰", "😍",
                            "🤔", "🙄", "😴", "🤯", "🥳", "😎",
                            // Hands & gestures.
                            "👍", "👎", "👏", "🙌", "🙏", "🤝", "✊", "💪",
                            // Symbols / objects.
                            "🔥", "💯", "⭐", "🎉", "✨", "💡", "🚀", "💀", "👀",
                            // Picker handle.
                            "∞",
                        ];
                        let is_own = msg.sender_key == state.profile_public_key;

                        // Estimated popup geometry — needed for the sticky
                        // hover region. Function buttons are now text labels
                        // (Pin / Edit) which are wider than the prior icon
                        // attempts, so widen the estimate accordingly.
                        let func_w = 26.0      // ↩ reply
                                   + 36.0      // Pin
                                   + (if is_own { 40.0 } else { 0.0 }); // Edit (own only)
                        let est_popup_w =
                            func_w
                            + 18.0                       // Þ separator
                            + TOP_REACTIONS.len() as f32 * 28.0 // top-10 reactions
                            + 30.0                       // ∞ button
                            + 16.0;                      // padding
                        // Popup rect adjacent to the pill (no gap so cursor
                        // sliding rightward stays in a connected hover region).
                        let est_popup_rect = egui::Rect::from_min_size(
                            egui::pos2(pill_rect_for_msg.right(), pill_rect_for_msg.top() - 2.0),
                            Vec2::new(est_popup_w, pill_rect_for_msg.height() + 4.0),
                        );
                        // Sticky hover gate: open if cursor is over THE PILL
                        // or over THE POPUP REGION specifically. Earlier code
                        // used pill.union(popup) which created a bounding
                        // rect that included the message TEXT area between
                        // them — so hovering message body opened the popup.
                        // (Operator-reported bug 2026-05-04.)
                        let pointer = ui.ctx().input(|i| i.pointer.hover_pos());
                        // Block the popup whenever a foreground modal is
                        // open over the chat. Egui's positional `.contains()`
                        // checks don't respect z-ordering, so without this
                        // gate the popup opens when the cursor is over a
                        // modal that happens to sit above a message's pill
                        // region — blocking the modal's own buttons.
                        let modal_blocking = state.chat_user_modal_open
                            || state.show_create_group_modal
                            || state.show_join_group_modal
                            || state.image_viewer_url.is_some();
                        // Þ-only hover detection — does NOT use the wider
                        // est_popup_rect because that overlaps the message
                        // text on the same line (operator feedback
                        // 2026-05-12 — "if I mouse over the text of a reply
                        // the reaction pill comes up even though I never
                        // clicked on the Þ"). Popups OPEN only via thorn.
                        let thorn_hovered = !modal_blocking
                            && thorn_rect_for_msg != egui::Rect::NOTHING
                            && pointer.map(|p| thorn_rect_for_msg.contains(p)).unwrap_or(false);
                        // popup_hovered is ONLY honored for sticky behavior
                        // once a popup is ALREADY open for this message
                        // (state.chat_open_popup_ts == Some(target_ts)).
                        // Otherwise hovering the message text right of the
                        // pill (which is geometrically inside est_popup_rect)
                        // would open the popup spuriously.
                        //
                        // Sticky region while popup is open INCLUDES the
                        // full pill_rect — not just est_popup_rect — so the
                        // cursor can travel rightward through the pill
                        // (past Þ, across timestamp text + existing reaction
                        // badges) on its way to the popup without falling
                        // into a "dead zone" that closes the popup.
                        // Operator feedback 2026-05-12, "the problem now
                        // is that I can't move the mouse off of the Þ
                        // without the reaction pill disappearing."
                        let popup_already_open_for_msg = state.chat_open_popup_ts == Some(target_ts);
                        let popup_hovered = !modal_blocking
                            && popup_already_open_for_msg
                            && pointer.map(|p| {
                                pill_rect_for_msg.contains(p) || est_popup_rect.contains(p)
                            }).unwrap_or(false);
                        let combined_hovered = thorn_hovered || popup_hovered;

                        // Update the open-popup tracker. Open on first Þ
                        // hover; clear when cursor leaves both Þ AND popup.
                        if thorn_hovered {
                            state.chat_open_popup_ts = Some(target_ts);
                        } else if popup_already_open_for_msg && !combined_hovered {
                            state.chat_open_popup_ts = None;
                        }

                        if combined_hovered {
                            // ── Timestamp expansion overlay (LEFT of Þ) ──
                            // Shows YYYY-MM-DD HH:MM:SS UTC anchored just to
                            // the left of the Þ. Operator feedback 2026-05-12.
                            // Rendered before the reaction popup (which goes
                            // on the RIGHT) so the two overlays appear
                            // symmetrically around the Þ pull tab.
                            if msg.timestamp_ms > 0 && thorn_rect_for_msg != egui::Rect::NOTHING {
                                let ts_overlay_pos = egui::pos2(
                                    thorn_rect_for_msg.left() - 4.0,
                                    thorn_rect_for_msg.center().y,
                                );
                                let full_ts = format_full_timestamp(msg.timestamp_ms);
                                egui::Area::new(egui::Id::new(("pill_ts_expand", msg.timestamp_ms)))
                                    .fixed_pos(ts_overlay_pos)
                                    .pivot(egui::Align2::RIGHT_CENTER)
                                    .order(egui::Order::Foreground)
                                    .interactable(false)
                                    .show(ui.ctx(), |ui| {
                                        Frame::none()
                                            .fill(theme.bg_card())
                                            .stroke(Stroke::new(1.0, theme.border()))
                                            .rounding(Rounding::same(8))
                                            .inner_margin(egui::Margin::symmetric(8, 4))
                                            .shadow(egui::epaint::Shadow {
                                                offset: [1, 2],
                                                blur: 6,
                                                spread: 0,
                                                color: Color32::from_black_alpha(80),
                                            })
                                            .show(ui, |ui| {
                                                ui.label(
                                                    RichText::new(full_ts)
                                                        .size(theme.small_size)
                                                        .color(theme.text_secondary())
                                                        .monospace(),
                                                );
                                            });
                                    });
                            }

                            let overlay_pos = egui::pos2(
                                pill_rect_for_msg.right(),
                                pill_rect_for_msg.center().y,
                            );
                            // Animated channeling color (RGB cycle / pulse / red on attack).
                            // Used for every glyph + the Þ separator so the popup feels
                            // alive and matches the active-page nav border.
                            let chan_time = ui.ctx().input(|i| i.time) as f32;
                            let chan_attack = state.attack_pulse_active;
                            let chan = crate::gui::pages::escape_menu::channeling_color(
                                theme, chan_time, chan_attack, theme.accent(),
                            );
                            ui.ctx().request_repaint();
                            egui::Area::new(egui::Id::new(("pill_expand", target_ts)))
                                .fixed_pos(overlay_pos)
                                .pivot(egui::Align2::LEFT_CENTER)
                                .order(egui::Order::Foreground)
                                .interactable(true)
                                .show(ui.ctx(), |ui| {
                                    Frame::none()
                                        .fill(theme.bg_card())
                                        .stroke(Stroke::new(1.0, theme.border()))
                                        .rounding(Rounding::same(8))
                                        .inner_margin(4.0)
                                        .shadow(egui::epaint::Shadow {
                                            offset: [1, 2],
                                            blur: 6,
                                            spread: 0,
                                            color: Color32::from_black_alpha(80),
                                        })
                                        .show(ui, |ui| {
                                            ui.spacing_mut().item_spacing.x = 2.0;
                                            ui.horizontal(|ui| {
                                                // Functions on the LEFT of Þ. Use plain
                                                // unicode arrows + ASCII letters so they
                                                // render reliably in the default font
                                                // (emoji glyphs like 📌 ✎ show as squares
                                                // without an emoji font installed).
                                                if ui.add(
                                                    egui::Button::new(RichText::new("↩").size(theme.font_size_body).color(chan))
                                                        .min_size(Vec2::new(26.0, 22.0))
                                                        .rounding(Rounding::same(4))
                                                ).on_hover_text("Reply").clicked() {
                                                    let preview = if msg.content.len() > 80 {
                                                        format!("{}…", &msg.content[..80])
                                                    } else { msg.content.clone() };
                                                    pending_reply = Some(crate::gui::ReplyContext {
                                                        sender_key: msg.sender_key.clone(),
                                                        sender_name: msg.sender_name.clone(),
                                                        preview,
                                                        timestamp_ms: target_ts,
                                                    });
                                                }
                                                if ui.add(
                                                    egui::Button::new(RichText::new("Pin").size(theme.font_size_small).color(chan))
                                                        .min_size(Vec2::new(34.0, 22.0))
                                                        .rounding(Rounding::same(4))
                                                ).on_hover_text("Pin message").clicked() {
                                                    pending_pins.push((msg.sender_key.clone(), msg.sender_name.clone(), msg.content.clone(), target_ts));
                                                }
                                                if is_own {
                                                    if ui.add(
                                                        egui::Button::new(RichText::new("Edit").size(theme.font_size_small).color(chan))
                                                            .min_size(Vec2::new(38.0, 22.0))
                                                            .rounding(Rounding::same(4))
                                                    ).on_hover_text("Edit message").clicked() {
                                                        pending_edit = Some((target_ts, msg.content.clone()));
                                                    }
                                                }

                                                // Þ separator (matches the inline pill).
                                                // Painted in the channeling color too so the
                                                // whole popup pulses together.
                                                ui.add_space(2.0);
                                                ui.label(RichText::new("Þ").size(theme.font_size_body).color(chan).strong());
                                                ui.add_space(2.0);

                                                // Top-10 reactions on the RIGHT of Þ. Strip
                                                // any U+FE0F variation selector — it ends up
                                                // as a tofu square next to ❤ in fonts that
                                                // don't honor the emoji presentation hint.
                                                for emoji in TOP_REACTIONS {
                                                    let clean: String = emoji.chars().filter(|c| *c != '\u{FE0F}').collect();
                                                    if ui.add(
                                                        egui::Button::new(RichText::new(&clean).size(theme.font_size_body))
                                                            .min_size(Vec2::new(26.0, 22.0))
                                                            .rounding(Rounding::same(4))
                                                    ).clicked() {
                                                        pending_reactions.push((target_from.clone(), target_ts, clean.clone()));
                                                    }
                                                }

                                                // ∞ all-emoji picker (popup with full set).
                                                let inf_resp = ui.add(
                                                    egui::Button::new(RichText::new("∞").size(theme.font_size_body).color(chan))
                                                        .min_size(Vec2::new(26.0, 22.0))
                                                        .rounding(Rounding::same(4))
                                                ).on_hover_text("All reactions");
                                                let inf_popup_id = egui::Id::new(("react_inf_popup", target_ts));
                                                if inf_resp.clicked() {
                                                    ui.memory_mut(|m| m.toggle_popup(inf_popup_id));
                                                }
                                                egui::popup::popup_below_widget(
                                                    ui, inf_popup_id, &inf_resp,
                                                    egui::PopupCloseBehavior::CloseOnClickOutside,
                                                    |ui| {
                                                        ui.set_min_width(280.0);
                                                        ui.horizontal_wrapped(|ui| {
                                                            for emoji in ALL_REACTIONS {
                                                                let clean: String = emoji.chars().filter(|c| *c != '\u{FE0F}').collect();
                                                                if ui.button(&clean).clicked() {
                                                                    pending_reactions.push((target_from.clone(), target_ts, clean.clone()));
                                                                    ui.memory_mut(|m| m.close_popup());
                                                                }
                                                            }
                                                        });
                                                    },
                                                );
                                            });
                                        });
                                });
                        }
                    }

                    // Render each attached image as a clickable thumbnail
                    // indented under the message text.
                    if !image_urls.is_empty() {
                        let server_url = state.server_url.clone();
                        const THUMB_INDENT: f32 = 40.0;
                        const THUMB_W: f32 = 240.0;
                        for raw_url in image_urls {
                            let url = crate::gui::widgets::image_cache::resolve_url(&raw_url, &server_url);
                            state.image_cache.request(&url);
                            let status = state.image_cache.status(&url);

                            match status {
                                crate::gui::widgets::image_cache::ImageStatus::Ready { width, height } => {
                                    let aspect = width as f32 / height.max(1) as f32;
                                    let thumb_h = (THUMB_W / aspect.max(0.1)).min(360.0).max(60.0);
                                    let row_w = ui.available_width();
                                    let (row_rect, resp) = ui.allocate_exact_size(
                                        Vec2::new(row_w, thumb_h + 4.0),
                                        egui::Sense::click(),
                                    );
                                    ui.painter().rect_filled(row_rect, 0.0, row_bg);
                                    if let Some(tex) = state.image_cache.get_texture(&url) {
                                        let img_rect = egui::Rect::from_min_size(
                                            egui::pos2(row_rect.left() + THUMB_INDENT, row_rect.top() + 2.0),
                                            Vec2::new(THUMB_W, thumb_h),
                                        );
                                        let mut mesh = egui::Mesh::with_texture(tex.id());
                                        mesh.add_rect_with_uv(
                                            img_rect,
                                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                            Color32::WHITE,
                                        );
                                        ui.painter().add(egui::Shape::mesh(mesh));
                                        // Faint border
                                        ui.painter().rect_stroke(
                                            img_rect,
                                            Rounding::same(3),
                                            Stroke::new(1.0, theme.border()),
                                            egui::StrokeKind::Inside,
                                        );
                                    }
                                    if resp.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    if resp.clicked() {
                                        state.image_viewer_url = Some(url.clone());
                                    }
                                }
                                crate::gui::widgets::image_cache::ImageStatus::Fetching
                                | crate::gui::widgets::image_cache::ImageStatus::Idle => {
                                    let row_w = ui.available_width();
                                    let (row_rect, _) = ui.allocate_exact_size(
                                        Vec2::new(row_w, 20.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(row_rect, 0.0, row_bg);
                                    ui.painter().text(
                                        egui::pos2(row_rect.left() + THUMB_INDENT, row_rect.center().y),
                                        egui::Align2::LEFT_CENTER,
                                        &format!("Loading image… {}", crate::gui::widgets::image_cache::filename_from_url(&url)),
                                        egui::FontId::proportional(theme.font_size_small),
                                        theme.text_muted(),
                                    );
                                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                                }
                                crate::gui::widgets::image_cache::ImageStatus::Failed(err) => {
                                    let row_w = ui.available_width();
                                    let (row_rect, _) = ui.allocate_exact_size(
                                        Vec2::new(row_w, 20.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(row_rect, 0.0, row_bg);
                                    ui.painter().text(
                                        egui::pos2(row_rect.left() + THUMB_INDENT, row_rect.center().y),
                                        egui::Align2::LEFT_CENTER,
                                        &format!("Image failed: {err}"),
                                        egui::FontId::proportional(theme.font_size_small),
                                        theme.danger(),
                                    );
                                }
                            }
                        }
                    }
                }

                ui.add_space(8.0);

                // ── Deferred avatar post-pass ──
                // Paint avatars AFTER all rows + reply rows + image rows have
                // rendered, so the avatar's bottom doesn't get clipped by
                // subsequent row bgs (header rows are now sized to the text,
                // not to the avatar, so 32×32 avatars often overflow). Painting
                // last puts them on top of any covering bg fill.
                let avatar_ctx_time = ui.ctx().input(|i| i.time);
                for avatar in &deferred_avatars {
                    crate::gui::widgets::row::paint_avatar(ui, theme, avatar, avatar_ctx_time);
                }

                // Apply pending reply selection (set chat composing context).
                if let Some(ctx) = pending_reply.take() {
                    state.chat_reply_to = Some(ctx);
                }

                // Apply pending edit-target click (open the inline editor for this message).
                if let Some((ts, draft)) = pending_edit.take() {
                    state.chat_edit_target = Some((ts, draft));
                }

                // Apply pending edit cancel (clear the inline editor).
                if pending_edit_cancel {
                    state.chat_edit_target = None;
                }

                // Apply pending edit save (send the WS edit message + clear edit target).
                if let Some((ts, new_content)) = pending_edit_save.take() {
                    if let Some(ref client) = state.ws_client {
                        if client.is_connected() {
                            let msg = serde_json::json!({
                                "type": "edit",
                                "from": state.profile_public_key,
                                "timestamp": ts,
                                "new_content": new_content.clone(),
                                "channel": state.chat_active_channel,
                            });
                            client.send(&msg.to_string());
                        }
                    }
                    // Optimistic local update so the UI shows the new text immediately.
                    for m in state.chat_messages.iter_mut() {
                        if m.sender_key == state.profile_public_key && m.timestamp_ms == ts {
                            m.content = new_content;
                            break;
                        }
                    }
                    state.chat_edit_target = None;
                }

                // Send pending report slash commands.
                for (sender_name, _content) in pending_reports {
                    if let Some(ref client) = state.ws_client {
                        if client.is_connected() {
                            let ts = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64;
                            let report_cmd = format!("/report {}", sender_name);
                            let mut m = serde_json::json!({
                                "type": "chat",
                                "from": state.profile_public_key,
                                "from_name": state.user_name,
                                "content": report_cmd,
                                "timestamp": ts,
                                "channel": state.chat_active_channel,
                            });
                            // Inc2.MED-1: Dilithium chat signature.
                            if let Some(seed) = state.private_key_bytes.as_ref() {
                                m["pq_signature"] = serde_json::Value::String(
                                    crate::net::identity::pq_sign_chat(seed, &report_cmd, ts)
                                );
                            }
                            client.send(&m.to_string());
                        }
                    }
                }

                // v0.281.0: send pending delete requests via WebSocket.
                // Protocol: RelayMessage::Delete { from, timestamp }. The
                // relay decides whether to honor based on requester's role
                // (own / admin / mod), so we don't gate locally beyond
                // hiding the menu entry for the non-eligible cases.
                for (from_key, ts) in pending_deletes {
                    if let Some(ref client) = state.ws_client {
                        if client.is_connected() {
                            let r = serde_json::json!({
                                "type": "delete",
                                "from": from_key,
                                "timestamp": ts,
                            });
                            client.send(&r.to_string());
                        }
                    }
                }

                // Send pending pin requests via WebSocket.
                for (from_key, from_name, content, ts) in pending_pins {
                    if let Some(ref client) = state.ws_client {
                        if client.is_connected() {
                            let r = serde_json::json!({
                                "type": "pin_request",
                                "from_key": from_key,
                                "from_name": from_name,
                                "content": content,
                                "timestamp": ts,
                                "channel": state.chat_active_channel,
                            });
                            client.send(&r.to_string());
                        }
                    }
                }

                // Apply any pending reaction sends collected during render.
                for (target_from, target_ts, emoji) in pending_reactions {
                    if let Some(ref client) = state.ws_client {
                        if client.is_connected() {
                            let from_name = if !state.user_name.is_empty() {
                                state.user_name.clone()
                            } else {
                                "Anonymous".to_string()
                            };
                            let r = serde_json::json!({
                                "type": "reaction",
                                "target_from": target_from,
                                "target_timestamp": target_ts,
                                "emoji": emoji,
                                "from": state.profile_public_key,
                                "from_name": from_name,
                                "channel": state.chat_active_channel,
                            });
                            client.send(&r.to_string());
                        }
                    }
                }
            });
    });

    // ── Input bar ──
    let input_rect = egui::Rect::from_min_size(
        egui::pos2(available.min.x, available.max.y - input_height),
        Vec2::new(available.width(), input_height),
    );

    ui.allocate_ui_at_rect(input_rect, |ui| {
        Frame::NONE
            .fill(Color32::from_rgb(25, 25, 30))
            .inner_margin(egui::Margin::symmetric(16, 8))
            .show(ui, |ui| {
                // Reply banner — only shown when a reply context is active.
                if let Some(ref reply) = state.chat_reply_to.clone() {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "↩ Replying to {}: {}",
                                reply.sender_name,
                                if reply.preview.len() > 60 {
                                    format!("{}…", &reply.preview[..60])
                                } else {
                                    reply.preview.clone()
                                }
                            ))
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                        );
                        if widgets::Button::ghost("X").show(ui, theme) {
                            state.chat_reply_to = None;
                        }
                    });
                }

                // v0.282.0 typing indicator. Prune entries older than 3s so
                // a typer who quit typing 5s ago doesn't linger forever, then
                // render the names of users still in the active channel. We
                // intentionally don't filter by channel here — typing events
                // don't carry channel info (relay-side rate-limit is per-user,
                // not per-channel), so showing all typers app-wide is the
                // truthful answer until the protocol carries channel.
                const TYPING_TTL: std::time::Duration = std::time::Duration::from_secs(3);
                let now_typing_tick = std::time::Instant::now();
                state.chat_typing_users.retain(|_, (_, t)| now_typing_tick.duration_since(*t) < TYPING_TTL);
                if !state.chat_typing_users.is_empty() {
                    let names: Vec<String> = state.chat_typing_users.values()
                        .map(|(name, _)| name.clone())
                        .collect();
                    let label = match names.len() {
                        0 => String::new(), // unreachable per the is_empty check above
                        1 => format!("{} is typing…", names[0]),
                        2 => format!("{} and {} are typing…", names[0], names[1]),
                        _ => format!("{} and {} others are typing…", names[0], names.len() - 1),
                    };
                    if !label.is_empty() {
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(
                                egui::RichText::new(label)
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted())
                                    .italics(),
                            );
                        });
                        // Repaint so the prune-on-render keeps the line fresh
                        // (otherwise a UI without other inputs would stick the
                        // indicator past its TTL until something else redrew).
                        ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
                    }
                }

                Frame::NONE
                    .fill(Color32::from_rgb(30, 30, 38))
                    .rounding(Rounding::same(theme.border_radius_lg as u8))
                    .stroke(Stroke::new(1.0, theme.border()))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let hint = if state.chat_active_channel.starts_with("dm:") {
                        let pk = &state.chat_active_channel[3..];
                        let name = state.chat_dms.iter().find(|d| d.user_key == pk)
                            .map(|d| d.user_name.as_str()).unwrap_or("user");
                        format!("Message {}", name)
                    } else if let Some(gid) = state.chat_active_channel.strip_prefix("p2pgroup:") {
                        let name = state.p2p_groups.iter().find(|g| g.group_id == gid)
                            .map(|g| g.name.as_str()).unwrap_or("group");
                        format!("Message {} (encrypted)", name)
                    } else if state.chat_active_channel.starts_with("group:") {
                        let gid = &state.chat_active_channel[6..];
                        let name = state.chat_groups.iter().find(|g| g.id == gid)
                            .map(|g| g.name.as_str()).unwrap_or("group");
                        format!("Message {}", name)
                    } else {
                        format!("Message #{}", state.chat_active_channel)
                    };
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut state.chat_input)
                            .desired_width(ui.available_width() - 104.0)
                            .hint_text(hint),
                    );

                    // v0.282.0 outgoing typing event. Fire when the input
                    // CHANGED (egui's response.changed() debounces to "actual
                    // edit, not focus/scroll") AND we haven't sent one in
                    // the last 3 seconds. Matches the relay's silent-drop
                    // rate limit so we never waste bandwidth on rejected
                    // sends. Skipped on empty input (clearing the box isn't
                    // "typing"). Skipped when not connected.
                    if response.changed() && !state.chat_input.is_empty() {
                        let now = std::time::Instant::now();
                        let should_send = state.chat_typing_last_sent
                            .map(|t| now.duration_since(t).as_secs() >= 3)
                            .unwrap_or(true);
                        if should_send {
                            if let Some(ref client) = state.ws_client {
                                if client.is_connected() {
                                    let m = serde_json::json!({
                                        "type": "typing",
                                        "from": state.profile_public_key,
                                        "from_name": state.user_name,
                                    });
                                    client.send(&m.to_string());
                                    state.chat_typing_last_sent = Some(now);
                                }
                            }
                        }
                    }

                    // (Clipboard image paste detection moved to the top of
                    // pub fn draw() in v0.233 because egui's TextEdit consumes
                    // the Ctrl+V key event via filtered_events() before any
                    // code below it can detect it. See `pub fn draw` for the
                    // working detection.)

                    // ── @mention autocomplete ──
                    // If the input ends with `@partial` (no whitespace after the @),
                    // show a popup of matching users from chat_users.
                    let mention_partial: Option<String> = {
                        let text = &state.chat_input;
                        if let Some(at_pos) = text.rfind('@') {
                            let after_at = &text[at_pos + 1..];
                            // No whitespace after the @ means we're still typing the mention.
                            if !after_at.contains(char::is_whitespace) && after_at.len() < 32 {
                                Some(after_at.to_string())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    };

                    // True if Enter was consumed this frame to pick a mention
                    // (so the message-send below must NOT also fire).
                    let mut mention_took_enter = false;

                    if let Some(partial) = mention_partial {
                        let partial_lower = partial.to_lowercase();
                        let matches: Vec<String> = state.chat_users.iter()
                            .filter(|u| u.name.to_lowercase().starts_with(&partial_lower))
                            .take(8)
                            .map(|u| u.name.clone())
                            .collect();

                        // NOTE: no `response.has_focus()` gate — clicking a
                        // popup row defocuses the TextEdit FIRST, which (under
                        // the old guard) removed the popup the same frame the
                        // click would land, so mouse-select never worked
                        // (operator-reported 2026-05-15). The popup is now
                        // scoped purely by "input ends in @partial with
                        // matches", which disappears naturally once a name is
                        // inserted (the @partial is gone).
                        if !matches.is_empty() {
                            // Clamp highlight index into range.
                            if state.chat_mention_index >= matches.len() {
                                state.chat_mention_index = 0;
                            }

                            // Keyboard nav. A focused single-line TextEdit's
                            // event filter has vertical_arrows:false, so
                            // Up/Down are NOT consumed by it — ui.input sees
                            // them. Enter triggers the TextEdit's lost_focus
                            // but the key_pressed flag is still readable.
                            let (k_up, k_down, k_enter) = ui.input(|i| (
                                i.key_pressed(egui::Key::ArrowUp),
                                i.key_pressed(egui::Key::ArrowDown),
                                i.key_pressed(egui::Key::Enter),
                            ));
                            if k_down {
                                state.chat_mention_index =
                                    (state.chat_mention_index + 1) % matches.len();
                            }
                            if k_up {
                                state.chat_mention_index =
                                    (state.chat_mention_index + matches.len() - 1) % matches.len();
                            }

                            // Selection can come from Enter (highlighted row)
                            // or a mouse click on any row. Computed into
                            // locals so the Area closure doesn't need to
                            // borrow `state`.
                            let cur_index = state.chat_mention_index;
                            let mut selected: Option<String> = None;
                            let mut new_index = cur_index;
                            if k_enter {
                                selected = matches.get(cur_index).cloned();
                            }

                            // Render the suggestion list as a foreground Area
                            // anchored just ABOVE the input. Highlighted row
                            // uses the accent fill; hover moves the highlight.
                            let row_h = 24.0_f32;
                            let area_h = matches.len() as f32 * row_h + 34.0;
                            let area_pos = egui::pos2(
                                response.rect.left(),
                                response.rect.top() - area_h - 4.0,
                            );
                            egui::Area::new(egui::Id::new("mention_autocomplete_area"))
                                .order(egui::Order::Foreground)
                                .fixed_pos(area_pos)
                                .show(ui.ctx(), |ui| {
                                    Frame::popup(ui.style()).show(ui, |ui| {
                                        ui.set_min_width(220.0);
                                        ui.label(
                                            RichText::new(format!("Mention: @{}", partial))
                                                .size(theme.font_size_small)
                                                .color(theme.text_muted()),
                                        );
                                        ui.separator();
                                        for (idx, name) in matches.iter().enumerate() {
                                            let is_sel = idx == cur_index;
                                            let btn = egui::Button::new(
                                                RichText::new(format!("@{}", name)).color(
                                                    if is_sel {
                                                        theme.text_on_accent()
                                                    } else {
                                                        theme.text_primary()
                                                    },
                                                ),
                                            )
                                            .fill(if is_sel {
                                                theme.accent()
                                            } else {
                                                Color32::TRANSPARENT
                                            })
                                            .min_size(Vec2::new(210.0, row_h - 2.0));
                                            let r = ui.add(btn);
                                            if r.hovered() {
                                                new_index = idx;
                                            }
                                            if r.clicked() {
                                                selected = Some(name.clone());
                                            }
                                        }
                                    });
                                });

                            state.chat_mention_index = new_index;

                            if let Some(name) = selected {
                                // Replace the trailing @partial with @name + space.
                                if let Some(at_pos) = state.chat_input.rfind('@') {
                                    state.chat_input.truncate(at_pos);
                                    state.chat_input.push('@');
                                    state.chat_input.push_str(&name);
                                    state.chat_input.push(' ');
                                }
                                state.chat_mention_index = 0;
                                // Move the text caret to the END of the input
                                // (just past the trailing space we appended).
                                // egui keeps the caret in TextEditState keyed
                                // by the widget id; without this the caret
                                // stays where it was (mid-name) so the next
                                // keystroke lands inside the inserted name
                                // (operator-reported 2026-05-15).
                                {
                                    let end = egui::text::CCursor::new(
                                        state.chat_input.chars().count(),
                                    );
                                    let mut tes = egui::text_edit::TextEditState::load(
                                        ui.ctx(), response.id,
                                    ).unwrap_or_default();
                                    tes.cursor.set_char_range(Some(
                                        egui::text::CCursorRange::one(end),
                                    ));
                                    tes.store(ui.ctx(), response.id);
                                }
                                // Always re-focus the input (whether the pick
                                // came from Enter, click, or arrow+Enter) so
                                // the caret move sticks and the user keeps
                                // typing seamlessly.
                                ui.memory_mut(|m| m.request_focus(response.id));
                                if k_enter {
                                    // Don't let the same Enter also send the
                                    // message.
                                    mention_took_enter = true;
                                }
                            }
                        }
                    }

                    // Suppress the message-send when Enter was just used to
                    // pick a mention from the autocomplete popup.
                    let enter_pressed = !mention_took_enter
                        && response.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter));

                    // Search button — opens the message search modal.
                    if widgets::Button::ghost("🔍").show(ui, theme) {
                        state.chat_search_open = true;
                    }

                    // Pins button — shows pin count + opens the pins modal.
                    let pin_count = state.chat_pins.get(&state.chat_active_channel).map(|p| p.len()).unwrap_or(0);
                    let pin_label = if pin_count > 0 { format!("📌 {}", pin_count) } else { "📌".to_string() };
                    if widgets::Button::ghost(&pin_label).show(ui, theme) {
                        state.chat_pins_open = true;
                    }

                    // Help button (?) - opens slash commands reference
                    if widgets::Button::ghost("?").show(ui, theme) {
                        state.show_help_modal = !state.show_help_modal;
                    }

                    let send_clicked = widgets::Button::primary("Send").show(ui, theme);

                    if (enter_pressed || send_clicked) && !state.chat_input.trim().is_empty() {
                        let content = state.chat_input.trim().to_string();
                        let channel = state.chat_active_channel.clone();
                        // Single timestamp used for both the WS send and the local echo
                        // so reaction-targeting (which keys on sender + ts) matches.
                        let ts = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;

                        // B3 fix (v0.199.0): track whether the WS send was
                        // aborted (DM unencryptable, awaiting modal confirm).
                        // If aborted we ALSO skip the local message push so
                        // the user doesn't see their own message echo for a
                        // message that didn't actually send.
                        let mut send_aborted = false;

                        // P2P groups send over HTTP (signed objects), not the
                        // WS relay. Intercept here: encrypt under the cached
                        // epoch key + POST group_msg_v1. On success the shared
                        // echo block below shows the message immediately and
                        // the 4s poll reconciles with the relay's stored copy;
                        // on failure we abort so no phantom echo appears.
                        let is_p2p_group = channel.starts_with("p2pgroup:");
                        if is_p2p_group && !send_p2p_group_message(state, &channel, &content) {
                            send_aborted = true;
                        }

                        // Send via WebSocket if connected (channels, DMs, legacy
                        // groups). Skipped for P2P groups — those went via HTTP
                        // just above.
                        if !is_p2p_group {
                        if let Some(ref client) = state.ws_client {
                            if client.is_connected() {

                                // Resolve display name: prefer user_name, fall back to peer list, then "Anonymous"
                                let display_name = if !state.user_name.is_empty() {
                                    state.user_name.clone()
                                } else if let Some(me) = state.chat_users.iter().find(|u| u.public_key == state.profile_public_key) {
                                    if !me.name.is_empty() && me.name != "Anonymous" { me.name.clone() } else { "Anonymous".to_string() }
                                } else {
                                    "Anonymous".to_string()
                                };

                                // v0.199.0 (B3 fix): build the WS payload as Option<String>
                                // so the DM branch can SKIP the send if encryption isn't
                                // possible. Previously the DM code silently fell back to
                                // plaintext with only a log line — that's a downgrade
                                // attack vector. Now we stash the unencryptable message
                                // in state.dm_unencrypted_confirm and a modal asks the
                                // user to explicitly confirm "Send unencrypted anyway"
                                // or cancel.
                                let json_str_opt: Option<String> = if channel.starts_with("dm:") {
                                    let partner_key = &channel[3..];
                                    // Try to encrypt. Fail-reason strings line up with
                                    // PendingUnencryptedDm::reason values in mod.rs.
                                    let encrypt_outcome: Result<(String, String), &'static str> =
                                        try_encrypt_dm(state, partner_key, &content);
                                    match encrypt_outcome {
                                        Ok((content_b64, nonce_b64)) => {
                                            // Encrypted — build the DM JSON normally.
                                            let dm_obj = serde_json::json!({
                                                "type": "dm",
                                                "from": state.profile_public_key,
                                                "from_name": display_name,
                                                "to": partner_key,
                                                "content": content_b64,
                                                "nonce": nonce_b64,
                                                "encrypted": true,
                                                "timestamp": ts,
                                            });
                                            Some(dm_obj.to_string())
                                        }
                                        Err(reason) => {
                                            // B3: refuse the silent plaintext fallback.
                                            // Stash for the confirmation modal. The
                                            // pending DM holds the original plaintext
                                            // + ts so the user's choice is preserved if
                                            // they confirm later.
                                            let partner_name = state.chat_dms.iter()
                                                .find(|d| d.user_key == partner_key)
                                                .map(|d| d.user_name.clone())
                                                .unwrap_or_else(|| {
                                                    let take = 8.min(partner_key.len());
                                                    partner_key[..take].to_string()
                                                });
                                            log::warn!(
                                                "DM to {} ({}) cannot be encrypted ({}). Asking user to confirm plaintext.",
                                                partner_name, partner_key, reason
                                            );
                                            state.dm_unencrypted_confirm = Some(crate::gui::PendingUnencryptedDm {
                                                partner_key: partner_key.to_string(),
                                                partner_name,
                                                content: content.clone(),
                                                timestamp_ms: ts,
                                                reason: reason.to_string(),
                                            });
                                            None
                                        }
                                    }
                                } else if channel.starts_with("group:") {
                                    // Group: send as type "group_msg"
                                    let group_id = &channel[6..];
                                    Some(serde_json::json!({
                                        "type": "group_msg",
                                        "group_id": group_id,
                                        "content": content,
                                    }).to_string())
                                } else {
                                    // Normal channel chat. Include reply_to if a thread context is active.
                                    let mut chat_obj = serde_json::json!({
                                        "type": "chat",
                                        "from": state.profile_public_key,
                                        "from_name": display_name,
                                        "content": content,
                                        "timestamp": ts,
                                        "channel": channel,
                                    });
                                    if let Some(ref r) = state.chat_reply_to {
                                        chat_obj["reply_to"] = serde_json::json!({
                                            "from": r.sender_key,
                                            "from_name": r.sender_name,
                                            "content": r.preview,
                                            "timestamp": r.timestamp_ms,
                                        });
                                    }
                                    // Inc2.MED-1: Dilithium chat signature
                                    // over `content\ntimestamp` (the relay
                                    // now rejects non-bot chat without it).
                                    if let Some(seed) = state.private_key_bytes.as_ref() {
                                        chat_obj["pq_signature"] = serde_json::Value::String(
                                            crate::net::identity::pq_sign_chat(seed, &content, ts)
                                        );
                                    }
                                    Some(chat_obj.to_string())
                                };
                                if let Some(json_str) = json_str_opt {
                                    crate::debug::push_debug(format!("WS >>> {}", json_str));
                                    client.send(&json_str);

                                    // Track timestamp for dedup when server echoes it back
                                    state.chat_sent_timestamps.push(ts);
                                    // Keep only last 20 timestamps
                                    if state.chat_sent_timestamps.len() > 20 {
                                        state.chat_sent_timestamps.remove(0);
                                    }
                                } else {
                                    // B3: send was aborted because DM is unencryptable.
                                    // Skip the local message push too — modal will
                                    // handle resend after user confirms.
                                    send_aborted = true;
                                }
                            }
                        }
                        } // end of `if !is_p2p_group` (WS send path)

                        if send_aborted {
                            // Don't add the local echo or clear input — the
                            // pending DM is in state.dm_unencrypted_confirm.
                            // Modal will deal with it on the next frame.
                            // Returning the closure (early-out via continue
                            // analog: just skip the rest of the if-block).
                        } else {

                        // Store locally so user sees their own message immediately
                        let now = chrono_now_str();
                        let local_name = if !state.user_name.is_empty() {
                            state.user_name.clone()
                        } else if let Some(me) = state.chat_users.iter().find(|u| u.public_key == state.profile_public_key) {
                            if !me.name.is_empty() && me.name != "Anonymous" { me.name.clone() } else { "You".to_string() }
                        } else {
                            "You".to_string()
                        };
                        let local_reply_to = state.chat_reply_to.clone();
                        state.chat_messages.push(ChatMessage {
                            sender_name: local_name,
                            sender_key: state.profile_public_key.clone(),
                            content,
                            timestamp: now,
                            timestamp_ms: ts,
                            channel,
                            reply_to: local_reply_to,
                            ..Default::default()
                        });
                        // Clear reply context — the reply has been sent.
                        state.chat_reply_to = None;

                        while state.chat_messages.len() > 200 {
                            state.chat_messages.remove(0);
                        }

                        state.chat_input.clear();
                        response.request_focus();
                        } // end of `else` branch added by B3 fix (send_aborted == false)
                    }

                    if enter_pressed {
                        response.request_focus();
                    }
                });
                });
            });
    });
}

// ─────────────────────────────── User Profile Modal ────────────────────────

fn draw_user_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    if !state.chat_user_modal_open { return; }

    let mut close_modal = false;

    egui::Window::new("User Profile")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(320.0, 0.0))
        .title_bar(false)
        .frame(Frame::NONE.fill(theme.bg_sidebar_dark()).inner_margin(20.0).rounding(Rounding::same(8)).stroke(Stroke::new(1.0, Color32::from_rgb(50, 50, 60))))
        .show(ctx, |ui| {
            let name = state.chat_user_modal_name.clone();
            let key = state.chat_user_modal_key.clone();

            // Title bar with close X
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("User Profile")
                        .size(theme.font_size_body)
                        .color(theme.text_muted()),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add(egui::Button::new(
                        RichText::new("X").size(theme.font_size_body).color(theme.text_secondary()),
                    ).fill(Color32::TRANSPARENT).frame(false)).clicked() {
                        close_modal = true;
                    }
                });
            });
            ui.add_space(8.0);

            // Avatar circle with initial
            ui.vertical_centered(|ui| {
                let icon_color = name_color(&name);
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(64.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 30.0, icon_color);
                let initial = name.chars().next().unwrap_or('?').to_uppercase().to_string();
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &initial,
                    egui::FontId::proportional(28.0),
                    Color32::WHITE,
                );
            });

            ui.add_space(8.0);

            // Display name (bold) + role badge
            let user_role = state.chat_users.iter()
                .find(|u| u.public_key == key)
                .map(|u| u.role.clone())
                .unwrap_or_default();

            ui.vertical_centered(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&name)
                            .size(theme.font_size_heading)
                            .color(theme.text_primary())
                            .strong(),
                    );
                    if !user_role.is_empty() && user_role != "member" {
                        draw_role_badges(ui, theme, &user_role, &state.chat_roles);
                    }
                });
            });

            ui.add_space(4.0);

            // Online/offline status - user is online if they appear in chat_users
            // (peer_list only contains connected users; full_user_list has an "online" field)
            let user_entry = state.chat_users.iter().find(|u| u.public_key == key);
            let user_status = user_entry
                .map(|u| u.status.clone())
                .unwrap_or_else(|| "offline".to_string());
            // If status is empty or unrecognized, and user exists in the list, default to online
            let effective_status = if user_status.is_empty() { "online".to_string() } else { user_status };
            ui.vertical_centered(|ui| {
                let (dot_color, status_text) = match effective_status.as_str() {
                    "offline" => (Color32::from_rgb(100, 100, 100), "Offline"),
                    "away" => (theme.warning(), "Away"),
                    "busy" | "dnd" => (theme.danger(), "Do Not Disturb"),
                    _ => (theme.success(), "Online"),
                };
                ui.horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 4.0, dot_color);
                    ui.label(
                        RichText::new(status_text)
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                });
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // Public key (truncated) with Copy button
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Key:")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
                let display_key = if key.len() > 16 {
                    format!("{}...{}", &key[..8], &key[key.len()-8..])
                } else {
                    key.clone()
                };
                ui.label(
                    RichText::new(&display_key)
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
                if ui.add(egui::Button::new(
                    RichText::new("Copy").size(theme.font_size_small - 1.0).color(theme.text_muted()),
                ).fill(Color32::from_rgb(45, 45, 55))).clicked() {
                    ui.ctx().copy_text(key.clone());
                }
            });

            ui.add_space(12.0);

            // Determine relationship state
            let is_following = state.chat_friends.iter().any(|f| f.public_key == key);

            // Check if target user is streaming
            let is_streaming = state.chat_users.iter()
                .find(|u| u.public_key == key)
                .map(|u| u.status == "streaming")
                .unwrap_or(false);

            // Action buttons
            let btn_width = 140.0;
            ui.vertical_centered(|ui| {
                // Row 1: Send DM (always) + Follow/Unfollow toggle
                ui.horizontal(|ui| {
                    if ui.add(
                        egui::Button::new(
                            RichText::new("Send DM")
                                .size(theme.font_size_body)
                                .color(theme.text_primary()),
                        )
                        .fill(Color32::from_rgb(45, 45, 55))
                        .min_size(Vec2::new(btn_width, 30.0)),
                    ).clicked() {
                        // Open DM with this user
                        let dm_channel = format!("dm:{}", key);
                        // Add to DMs list if not already there
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
                            state.chat_active_channel = dm_channel;
                            state.chat_messages.clear();
                            state.history_fetched = false;
                            if let Some(ref client) = state.ws_client {
                                if client.is_connected() {
                                    let msg = serde_json::json!({
                                        "type": "dm_open",
                                        "partner": key,
                                    });
                                    client.send(&msg.to_string());
                                }
                            }
                        }
                        close_modal = true;
                    }

                    ui.add_space(4.0);

                    if is_following {
                        if ui.add(
                            egui::Button::new(
                                RichText::new("Unfollow")
                                    .size(theme.font_size_body)
                                    .color(theme.text_secondary()),
                            )
                            .fill(Color32::from_rgb(50, 35, 35))
                            .min_size(Vec2::new(btn_width, 30.0)),
                        ).clicked() {
                            if let Some(ref client) = state.ws_client {
                                if client.is_connected() {
                                    let msg = serde_json::json!({
                                        "type": "unfollow",
                                        // Field is "target_key" — must match
                                        // RelayMessage::Unfollow + the web
                                        // client. Was "target" (silently
                                        // dropped by the relay; Unfollow
                                        // never worked). Fixed v0.243.
                                        "target_key": key,
                                    });
                                    client.send(&msg.to_string());
                                }
                            }
                            // Remove from local friends list immediately
                            state.chat_friends.retain(|f| f.public_key != key);
                        }
                    } else {
                        if ui.add(
                            egui::Button::new(
                                RichText::new("Follow")
                                    .size(theme.font_size_body)
                                    .color(theme.text_on_accent()),
                            )
                            .fill(theme.accent())
                            .min_size(Vec2::new(btn_width, 30.0)),
                        ).clicked() {
                            if let Some(ref client) = state.ws_client {
                                if client.is_connected() {
                                    let msg = serde_json::json!({
                                        "type": "follow",
                                        // Field is "target_key" — must match
                                        // RelayMessage::Follow + the web
                                        // client. Was "target" (silently
                                        // dropped by the relay; Follow never
                                        // worked from the native app). v0.243.
                                        "target_key": key,
                                    });
                                    client.send(&msg.to_string());
                                }
                            }
                        }
                    }
                });

                // Watch Stream (only if user is streaming)
                if is_streaming {
                    ui.add_space(4.0);
                    if ui.add(
                        egui::Button::new(
                            RichText::new("Watch Stream")
                                .size(theme.font_size_body)
                                .color(theme.text_on_accent()),
                        )
                        .fill(Color32::from_rgb(100, 50, 150))
                        .min_size(Vec2::new(btn_width * 2.0 + 4.0, 30.0)),
                    ).clicked() {
                        // Placeholder for stream watching
                    }
                }

                // Moderator section (only if viewer is mod or admin)
                let my_role = viewer_role(state);
                let is_mod = my_role == "moderator" || my_role == "mod" || my_role == "admin";
                let is_admin = my_role == "admin";

                if is_mod {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Moderation")
                            .size(theme.font_size_small)
                            .color(theme.warning())
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.add(
                            egui::Button::new(
                                RichText::new("Mute")
                                    .size(theme.font_size_body)
                                    .color(theme.text_primary()),
                            )
                            .fill(Color32::from_rgb(60, 50, 30))
                            .min_size(Vec2::new(btn_width, 28.0)),
                        ).clicked() {
                            if let Some(ref client) = state.ws_client {
                                if client.is_connected() {
                                    let msg = serde_json::json!({
                                        "type": "mod_action",
                                        "action": "mute",
                                        "target": key,
                                        "target_name": name,
                                    });
                                    client.send(&msg.to_string());
                                }
                            }
                        }

                        ui.add_space(4.0);

                        if ui.add(
                            egui::Button::new(
                                RichText::new("Kick")
                                    .size(theme.font_size_body)
                                    .color(theme.text_primary()),
                            )
                            .fill(Color32::from_rgb(70, 40, 30))
                            .min_size(Vec2::new(btn_width, 28.0)),
                        ).clicked() {
                            if let Some(ref client) = state.ws_client {
                                if client.is_connected() {
                                    let msg = serde_json::json!({
                                        "type": "mod_action",
                                        "action": "kick",
                                        "target": key,
                                        "target_name": name,
                                    });
                                    client.send(&msg.to_string());
                                }
                            }
                        }
                    });
                }

                // Admin section (only if viewer is admin)
                if is_admin {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Admin")
                            .size(theme.font_size_small)
                            .color(theme.danger())
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.add(
                            egui::Button::new(
                                RichText::new("Ban")
                                    .size(theme.font_size_body)
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::from_rgb(120, 30, 30))
                            .min_size(Vec2::new(90.0, 28.0)),
                        ).clicked() {
                            if let Some(ref client) = state.ws_client {
                                if client.is_connected() {
                                    let msg = serde_json::json!({
                                        "type": "mod_action",
                                        "action": "ban",
                                        "target": key,
                                        "target_name": name,
                                    });
                                    client.send(&msg.to_string());
                                }
                            }
                        }

                        ui.add_space(4.0);

                        let target_is_mod = user_role == "moderator" || user_role == "mod";
                        let mod_btn_label = if target_is_mod { "Unmod" } else { "Mod" };
                        if ui.add(
                            egui::Button::new(
                                RichText::new(mod_btn_label)
                                    .size(theme.font_size_body)
                                    .color(theme.text_primary()),
                            )
                            .fill(Color32::from_rgb(50, 50, 70))
                            .min_size(Vec2::new(90.0, 28.0)),
                        ).clicked() {
                            if let Some(ref client) = state.ws_client {
                                if client.is_connected() {
                                    let action = if target_is_mod { "unmod" } else { "mod" };
                                    let msg = serde_json::json!({
                                        "type": "mod_action",
                                        "action": action,
                                        "target": key,
                                        "target_name": name,
                                    });
                                    client.send(&msg.to_string());
                                }
                            }
                        }
                    });

                    // ── Role assignment dropdown (roles Phase R2) ──
                    // Lists every role from the relay's role_list. Picking
                    // one sends set_user_role. This is the operator's
                    // chosen assignment path for custom roles (e.g. give
                    // dad a "Family" role with can_stream). Mod/Unmod above
                    // stay as quick shortcuts.
                    if !state.chat_roles.is_empty() {
                        ui.add_space(6.0);
                        let roles = state.chat_roles.clone();
                        let current_label = roles
                            .iter()
                            .find(|r| r.id == user_role)
                            .map(|r| r.label.clone())
                            .unwrap_or_else(|| {
                                if user_role.is_empty() { "Unverified".to_string() }
                                else { user_role.clone() }
                            });
                        let mut picked: Option<String> = None;
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Role:")
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted()),
                            );
                            egui::ComboBox::from_id_salt(("user_role_combo", key.as_str()))
                                .selected_text(current_label)
                                .show_ui(ui, |ui| {
                                    for r in &roles {
                                        // Label in the role's own badge
                                        // color so custom roles read
                                        // distinctly. selectable_label's
                                        // highlight conveys the current
                                        // selection (no glyph needed —
                                        // keeps icon_glyph_lint happy).
                                        let sel = r.id == user_role;
                                        if ui.selectable_label(
                                            sel,
                                            RichText::new(&r.label)
                                                .color(parse_role_color(&r.color, theme)),
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

                ui.add_space(8.0);

                if ui.add(
                    egui::Button::new(
                        RichText::new("Close")
                            .size(theme.font_size_body)
                            .color(theme.text_secondary()),
                    )
                    .fill(Color32::from_rgb(40, 40, 48))
                    .min_size(Vec2::new(btn_width * 2.0 + 4.0, 28.0)),
                ).clicked() {
                    close_modal = true;
                }
            });
        });
    if close_modal {
        state.chat_user_modal_open = false;
    }
}

// ─────────────────────────────── Create Channel Modal ──────────────────────

fn draw_create_channel_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let mut open = state.show_create_channel_modal;
    widgets::dialog(ctx, theme, "create_channel_dialog", "Create Channel", &mut open, |ui| {
        ui.set_min_width(300.0);

        widgets::form_row(ui, theme, "Channel name", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.new_channel_name)
                    .desired_width(220.0)
                    .hint_text("e.g. announcements"),
            );
        });

        widgets::form_row(ui, theme, "Description", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.new_channel_description)
                    .desired_width(220.0)
                    .hint_text("What is this channel about?"),
            );
        });

        ui.add_space(theme.spacing_md);

        ui.horizontal(|ui| {
            let name_valid = !state.new_channel_name.trim().is_empty();
            ui.add_enabled_ui(name_valid, |ui| {
                if widgets::Button::primary("Create").show(ui, theme) {
                    if let Some(ref client) = state.ws_client {
                        if client.is_connected() {
                            let msg = serde_json::json!({
                                "type": "channel_create",
                                "name": state.new_channel_name.trim(),
                                "description": state.new_channel_description.trim(),
                            });
                            client.send(&msg.to_string());
                            log::info!("Channel create requested: {}", state.new_channel_name.trim());
                            crate::debug::push_debug(format!("Channel create: {}", state.new_channel_name.trim()));
                        }
                    }
                    state.show_create_channel_modal = false;
                }
            });
            ui.add_space(theme.spacing_sm);
            if widgets::Button::secondary("Cancel").show(ui, theme) {
                state.show_create_channel_modal = false;
            }
        });
    });
    // Apply X-button close back to state.
    if !open {
        state.show_create_channel_modal = false;
    }
}

// ─────────────────────────────── Add Server Modal ───────────────────────

fn draw_add_server_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
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
        let url = state.add_server_url_draft.trim();
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

fn draw_edit_channel_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
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

fn draw_create_group_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
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
            ui.add(
                egui::TextEdit::multiline(&mut display)
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

fn draw_join_group_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
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
                ui.add(
                    egui::TextEdit::multiline(&mut state.join_group_invite_code)
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

// ─────────────────────────── P2P Group (inline chat) ─────────────────────
// A P2P group opens like a channel: clicking it sets the active channel to
// "p2pgroup:<id>" and its decrypted messages render in the SAME center panel
// as channels and DMs (no modal — operator: switching to a group should feel
// like switching from #general to #announcements). These helpers do the
// network + crypto work (blocking ureq, same pattern as image upload):
//   enter_p2p_group — full sync on open: rekey-if-creator, fetch+unseal the
//                      epoch key, load the roster name map, decrypt history.
//   poll_p2p_group  — light 4s refresh: re-decrypt the log with the cached
//                      key (skips the key exchange + roster fetch).
// Both project decrypted GroupMessages into state.chat_messages tagged with
// the "p2pgroup:<id>" channel so the standard message renderer handles them
// (identicons, sender grouping, theme — all reused, zero parallel UI).

/// Format an epoch-ms timestamp as HH:MM:SS (UTC) for the message row.
/// Replace cached messages for `channel` with the freshly-decrypted set, while
/// preserving any of my just-sent local echoes the reload hasn't indexed yet.
/// Repaints from the relay's authoritative log (the source of truth) so edits/
/// removals elsewhere converge.
fn replace_p2p_messages(
    state: &mut GuiState,
    channel: &str,
    msgs: Vec<crate::net::api_v2::GroupMessage>,
) {
    let my_key = state.profile_public_key.clone();
    // My author fingerprint = BLAKE3(my Dilithium pubkey)[..16]. profile_public_key
    // IS the Dilithium pubkey hex (set in mod.rs from the PQ identity), so we
    // compute it with a cheap hex-decode + hash instead of derive_pq_identity —
    // which ran a full Dilithium + Kyber KEYGEN on every poll-apply (~ every 4s)
    // purely to recover a value we already have. Same result, no keygen churn.
    let my_fp = hex::decode(&my_key)
        .ok()
        .map(|b| crate::net::api_v2::author_fingerprint_hex(&b))
        .unwrap_or_default();

    // Preserve very-recent messages the reload hasn't indexed yet — so a poll
    // that races the relay (the author's POST, or a peer's mesh push that the
    // relay hasn't stored yet) doesn't blink a just-shown message out (the
    // "briefly disappeared then reappeared" the operator saw). Two sources:
    //   (a) my own optimistic local echoes (sender_key == my_key), and
    //   (b) inc-2 peer messages rendered from a WebRTC mesh push (handle_p2p_
    //       group_obj), which the 2s relay poll may not have indexed yet.
    // A pending message is kept only until the reload includes it (matched by
    // author fingerprint + content) and at most ~20s (a never-stored message
    // then falls off instead of lingering forever). For a peer message we map
    // its sender_key (pubkey hex) → fingerprint to compare against the reload.
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let pending: Vec<ChatMessage> = state
        .chat_messages
        .iter()
        .filter(|m| {
            if m.channel != channel || now_ms.saturating_sub(m.timestamp_ms) >= 20_000 {
                return false;
            }
            // Fingerprint for this message's author: cheap hex-decode + hash of
            // the sender_key (== my_fp for my own echoes; the peer's fp for a
            // pushed peer message). Falls back to empty (never matches) on a
            // non-hex sender_key, which just means we keep it until it ages out.
            let author_fp = if m.sender_key == my_key {
                my_fp.clone()
            } else {
                hex::decode(&m.sender_key)
                    .ok()
                    .map(|b| crate::net::api_v2::author_fingerprint_hex(&b))
                    .unwrap_or_default()
            };
            // Keep it only if the reload doesn't already contain it.
            !msgs.iter().any(|lm| lm.author_fp == author_fp && lm.text == m.content)
        })
        .cloned()
        .collect();

    state.chat_messages.retain(|m| m.channel != channel);
    for m in msgs {
        // inc-2: mark every poll-loaded object_id seen so a mesh push that
        // arrives AFTER the poll already rendered this object (peer's relay POST
        // beat their push to us) is deduped by handle_p2p_group_obj instead of
        // double-rendering. The poll itself rebuilds from the authoritative relay
        // log and never consults the seen-set, so this can't suppress the poll.
        #[cfg(feature = "native")]
        if !m.object_id.is_empty() {
            state.p2p_group_seen_obj_ids.insert(m.object_id.clone());
        }
        let is_me = !my_fp.is_empty() && m.author_fp == my_fp;
        let sender_key = if is_me {
            my_key.clone()
        } else {
            state
                .p2p_group_fp_to_key
                .get(&m.author_fp)
                .cloned()
                .unwrap_or_else(|| m.author_fp.clone())
        };
        let sender_name = if is_me {
            if !state.user_name.is_empty() { state.user_name.clone() } else { "You".to_string() }
        } else {
            state
                .p2p_group_fp_to_name
                .get(&m.author_fp)
                .cloned()
                .unwrap_or_else(|| format!("{}…", &m.author_fp[..12.min(m.author_fp.len())]))
        };
        // Use the app-standard "HH:MM UTC" formatter (same as chrono_now_str,
        // which the local send-echo uses) so a message keeps ONE timestamp
        // format — fixes the echo's HH:MM flipping to HH:MM:SS on reconcile.
        let ts_str = format_timestamp(m.created_at as u64);
        state.chat_messages.push(ChatMessage {
            sender_name,
            sender_key,
            content: m.text,
            timestamp: ts_str,
            timestamp_ms: m.created_at as u64,
            channel: channel.to_string(),
            ..Default::default()
        });
    }
    // Re-add my preserved local echoes (just-sent, not yet in the reload).
    for p in pending {
        state.chat_messages.push(p);
    }
    // Global sort by ms keeps each channel's relative order correct (the
    // renderer filters by channel + iterates in vec order). Cross-channel
    // interleave is irrelevant since only one channel renders at a time.
    state.chat_messages.sort_by_key(|m| m.timestamp_ms);
    while state.chat_messages.len() > 400 { state.chat_messages.remove(0); }

    // inc-2: bound the dedup set so a very long single-group session can't grow
    // it without limit. HashSet has no ordering to evict by, and the poll just
    // re-inserted every currently-loaded (authoritative) object_id above, so if
    // it has ballooned far past the message cap we simply clear it. The only
    // cost is that a push which raced THIS exact rebuild could re-render once;
    // the next 2s poll collapses it. (In practice this branch ~never fires —
    // the set is reset on every group switch.)
    #[cfg(feature = "native")]
    if state.p2p_group_seen_obj_ids.len() > 4000 {
        state.p2p_group_seen_obj_ids.clear();
    }
}

/// Spawn a BACKGROUND load of `group_id` (rekey/epoch/roster/messages) and
/// stash the receiver. The UI never blocks: `drain_p2p_loaders` applies the
/// result when the worker returns. `fresh` = true for a click (resets cached
/// state + shows "Loading…"); false for a periodic refresh (keeps the current
/// view until new data arrives, so polling doesn't flicker).
pub(crate) fn spawn_group_load(state: &mut GuiState, group_id: &str, fresh: bool) {
    let server_url = state.server_url.clone();
    let seed = match state.private_key_bytes.clone() {
        Some(s) if !s.is_empty() => s,
        _ => {
            state.p2p_group_invite_status = "Connect first, no identity loaded.".to_string();
            return;
        }
    };
    state.p2p_group_active_id = group_id.to_string();
    state.p2p_group_last_fetch = Some(std::time::Instant::now());
    if fresh {
        // Clear cached state so a stale prior group's key/maps can't leak into
        // the loading view; show the spinner hint until the worker returns.
        state.p2p_group_chat_epoch = 0;
        state.p2p_group_chat_epoch_key = None;
        state.p2p_group_fp_to_key.clear();
        state.p2p_group_fp_to_name.clear();
        state.p2p_group_loading = true;
        // inc-2: fresh dedup set per opened group (mirror web's clear on switch),
        // so object_ids from a prior group can't suppress a new group's messages.
        #[cfg(feature = "native")]
        state.p2p_group_seen_obj_ids.clear();
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let gid = group_id.to_string();
    std::thread::spawn(move || {
        let load = crate::net::api_v2::load_group_blocking(&server_url, &seed, &gid);
        let _ = tx.send(load); // receiver may be gone if the user switched away
    });
    state.p2p_group_loader = Some((group_id.to_string(), rx));
}

/// Apply a finished background `GroupLoad` to state (main thread): cache the
/// epoch key + roster maps and repaint the message log. Ignored if the user
/// has since switched to a different group.
fn apply_group_load(state: &mut GuiState, load: crate::net::api_v2::GroupLoad) {
    if state.p2p_group_active_id != load.group_id {
        return; // stale — user switched away before this returned
    }
    let channel = format!("p2pgroup:{}", load.group_id);
    state.p2p_group_chat_epoch = load.epoch;
    state.p2p_group_chat_epoch_key = load.epoch_key;
    // Rebuild the fp → pubkey / name maps (name resolved here on the main
    // thread, where chat_users is available).
    state.p2p_group_fp_to_key.clear();
    state.p2p_group_fp_to_name.clear();
    for (fp, pubkey_hex) in &load.members {
        let name = state
            .chat_users
            .iter()
            .find(|u| u.public_key == *pubkey_hex)
            .map(|u| u.name.clone())
            .unwrap_or_else(|| format!("{}…", &pubkey_hex[..8.min(pubkey_hex.len())]));
        state.p2p_group_fp_to_name.insert(fp.clone(), name);
        state.p2p_group_fp_to_key.insert(fp.clone(), pubkey_hex.clone());
    }
    replace_p2p_messages(state, &channel, load.messages);
    state.p2p_group_loading = false;
    // inc-2: now that the roster is known, open/maintain DataChannels to every
    // member so subsequent messages arrive P2P (low-latency). Runs on every load
    // (initial + each 2s refresh) so the mesh tracks roster changes; offer_to is
    // idempotent for already-connected peers.
    #[cfg(feature = "native")]
    ensure_group_mesh(state);
}

/// Spawn a BACKGROUND refresh of the whole P2P-group list (left rail + member
/// counts). Keeps the list fresh when membership changes on another client and
/// lets us detect when the open group was disbanded/left elsewhere.
pub(crate) fn spawn_groups_list_refresh(state: &mut GuiState) {
    if state.p2p_groups_list_loader.is_some() {
        return; // one in flight already
    }
    let server_url = state.server_url.clone();
    let seed = match state.private_key_bytes.clone() {
        Some(s) if !s.is_empty() => s,
        _ => return,
    };
    let dilithium_hex = match crate::net::identity::derive_pq_identity(&seed) {
        Ok(id) => id.dilithium_hex,
        Err(_) => return,
    };
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if let Ok(list) = crate::net::api_v2::fetch_p2p_groups(&server_url, &dilithium_hex) {
            let _ = tx.send(list);
        }
    });
    state.p2p_groups_list_loader = Some(rx);
}

/// Drain any finished background loaders (called once at the top of draw).
/// Applies the active-group load + the group-list refresh, and exits a group
/// that vanished from the list (disbanded or left on another device).
pub(crate) fn drain_p2p_loaders(state: &mut GuiState) {
    // (1) Active-group load.
    if let Some((_gid, rx)) = &state.p2p_group_loader {
        match rx.try_recv() {
            Ok(load) => {
                state.p2p_group_loader = None;
                apply_group_load(state, load);
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.p2p_group_loader = None; // worker died without sending
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }
    // (2) Group-list refresh.
    if let Some(rx) = &state.p2p_groups_list_loader {
        match rx.try_recv() {
            Ok(list) => {
                state.p2p_groups_list_loader = None;
                state.p2p_groups = list;
                // If the open group is gone (disbanded or we left it elsewhere),
                // exit to #general and drop its decrypted history.
                if let Some(agid) = state
                    .chat_active_channel
                    .strip_prefix("p2pgroup:")
                    .map(|s| s.to_string())
                {
                    if !state.p2p_groups.iter().any(|g| g.group_id == agid) {
                        let channel = format!("p2pgroup:{}", agid);
                        state.chat_messages.retain(|m| m.channel != channel);
                        state.chat_active_channel = "general".to_string();
                        state.p2p_group_active_id.clear();
                        state.p2p_group_chat_epoch_key = None;
                        state.p2p_group_loading = false;
                    }
                }
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.p2p_groups_list_loader = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }
}

/// Mint a fresh 7-day invite ticket for `group_id` and copy it to the
/// clipboard. Sets a transient status line shown in the group header.
fn mint_and_copy_p2p_invite(
    ctx: &egui::Context,
    state: &mut GuiState,
    group_id: &str,
    group_name: &str,
) {
    let server_url = state.server_url.clone();
    let seed = match state.private_key_bytes.clone() {
        Some(s) if !s.is_empty() => s,
        _ => {
            state.p2p_group_invite_status = "Connect first, no identity loaded.".to_string();
            return;
        }
    };
    let mut secret = vec![0u8; 32];
    use rand::RngCore;
    rand::rng().fill_bytes(&mut secret);
    let secret_hash = blake3::hash(&secret).as_bytes().to_vec();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let expires_at = now + 7 * 24 * 3600 * 1000;
    match crate::net::api_v2::submit_group_invite_v1(&server_url, &seed, group_id, expires_at, &secret_hash) {
        Ok(invite_id) => {
            let ticket = crate::net::api_v2::encode_invite_ticket(group_id, group_name, &invite_id, &secret);
            ctx.copy_text(ticket);
            state.p2p_group_invite_status = "Invite ticket copied, share within 7 days.".to_string();
        }
        Err(e) => {
            state.p2p_group_invite_status = format!("Invite failed: {e}");
        }
    }
}

/// Leave a P2P group (self-remove from the roster). Submits a
/// `group_member_v1` remove for my own key, then drops the view back to
/// #general and refreshes the group list so the row disappears.
fn leave_p2p_group(state: &mut GuiState, group_id: &str) {
    let server_url = state.server_url.clone();
    let seed = match state.private_key_bytes.clone() {
        Some(s) if !s.is_empty() => s,
        _ => {
            state.p2p_group_invite_status = "Connect first, no identity loaded.".to_string();
            return;
        }
    };
    match crate::net::api_v2::submit_group_leave(&server_url, &seed, group_id) {
        Ok(()) => {
            // Drop any cached messages for this group + leave the view.
            let channel = format!("p2pgroup:{}", group_id);
            state.chat_messages.retain(|m| m.channel != channel);
            state.chat_active_channel = "general".to_string();
            state.p2p_group_active_id.clear();
            state.p2p_group_chat_epoch_key = None;
            state.p2p_group_loader = None; // cancel any in-flight load for the left group
            state.p2p_group_loading = false;
            state.p2p_group_invite_status.clear();
            refresh_p2p_groups(state);
        }
        Err(e) => {
            state.p2p_group_invite_status = format!("Leave failed: {e}");
        }
    }
}

/// Disband a P2P group I created (creator-only `group_disband_v1`). Removes it
/// for everyone, drops the view back to #general, and refreshes the list.
fn disband_p2p_group(state: &mut GuiState, group_id: &str) {
    let server_url = state.server_url.clone();
    let seed = match state.private_key_bytes.clone() {
        Some(s) if !s.is_empty() => s,
        _ => {
            state.p2p_group_invite_status = "Connect first, no identity loaded.".to_string();
            return;
        }
    };
    match crate::net::api_v2::submit_group_disband(&server_url, &seed, group_id) {
        Ok(()) => {
            let channel = format!("p2pgroup:{}", group_id);
            state.chat_messages.retain(|m| m.channel != channel);
            state.chat_active_channel = "general".to_string();
            state.p2p_group_active_id.clear();
            state.p2p_group_chat_epoch_key = None;
            state.p2p_group_loader = None; // cancel any in-flight load for the disbanded group
            state.p2p_group_loading = false;
            state.p2p_group_invite_status.clear();
            refresh_p2p_groups(state);
        }
        Err(e) => {
            state.p2p_group_invite_status = format!("Disband failed: {e}");
        }
    }
}

/// Dispatch a P2P group message: do the synchronous pre-checks (we have a
/// group id, a seed, and an epoch key), then encrypt + POST on a BACKGROUND
/// thread so the UI never blocks on the network round-trip. Returns true if the
/// send was *dispatched* (caller then shows the optimistic echo + clears the
/// input); false only if a pre-check failed (no key/seed — caller aborts the
/// echo and the status line explains why).
///
/// Failure of the background POST is rare (network) and self-reconciles: the
/// optimistic echo is dropped on the next ~4s poll because the relay won't
/// serve back a message it never stored. (Previously this blocked the UI
/// thread on EVERY group message send — tens-to-hundreds of ms per message.)
fn send_p2p_group_message(state: &mut GuiState, channel: &str, content: &str) -> bool {
    let gid = match channel.strip_prefix("p2pgroup:") {
        Some(g) => g.to_string(),
        None => return false,
    };
    let server_url = state.server_url.clone();
    let seed = match state.private_key_bytes.clone() {
        Some(s) if !s.is_empty() => s,
        _ => {
            state.p2p_group_invite_status = "Connect first, no identity loaded.".to_string();
            return false;
        }
    };
    let key = match state.p2p_group_chat_epoch_key.clone() {
        Some(k) => k,
        None => {
            state.p2p_group_invite_status =
                "No epoch key yet, the creator must open the group once first.".to_string();
            return false;
        }
    };
    let epoch = state.p2p_group_chat_epoch.max(1);
    state.p2p_group_invite_status.clear();
    let content = content.to_string();

    // inc-2: build+sign the group_msg_v1 ONCE on the main thread so we have its
    // object_id + submission JSON for the mesh push and the seen-set. (Signing is
    // a few ms of Dilithium — fine inline; the network POST is what we defer to a
    // background thread.) Mirrors web `sendGroupMessage`: build → broadcast →
    // POST. If the build fails we abort the send (no echo) like the web client.
    let (object_id, submission_json) = match crate::net::api_v2::build_group_msg_submission(
        &seed, &gid, epoch, &key, &content,
    ) {
        Ok(pair) => pair,
        Err(e) => {
            log::warn!("group message build failed: {e}");
            state.p2p_group_invite_status = format!("Send failed: {e}");
            return false;
        }
    };

    // Push P2P FIRST (instant for connected roster members), then POST to the
    // relay (durable cache + offline backfill). Mark our own object seen so the
    // 2s poll doesn't re-handle it as if it were a peer push. (This whole file is
    // native-gated, so the mesh handle + seen-set always exist here.)
    state.p2p_group_seen_obj_ids.insert(object_id.clone());
    broadcast_group_obj(state, &gid, &submission_json);

    // Relay POST on a BACKGROUND thread so the UI never blocks on the round-trip.
    std::thread::spawn(move || {
        if let Err(e) = crate::net::api_v2::post_submission_json(&server_url, &submission_json) {
            log::warn!("group message relay POST failed (push may still have delivered; echo drops on next poll if not stored): {e}");
        }
    });
    true
}

/// inc-2: open/maintain WebRTC DataChannels to the active group's roster so
/// pushed messages arrive P2P (low-latency); the 2s relay poll remains the
/// offline backfill + source of truth. Mirrors web `ensureGroupMesh`.
///
/// The manager's offerer rule (only `my_key > peer` actually offers) handles
/// glare, so we just call `offer_to` for EVERY roster member — offline members
/// simply never connect (their offer goes nowhere), and an already-open/-pending
/// channel is a cheap no-op inside the manager. Called from the group-load apply
/// and the periodic refresh, so the mesh tracks roster changes.
#[cfg(feature = "native")]
pub(crate) fn ensure_group_mesh(state: &GuiState) {
    let webrtc = match &state.webrtc {
        Some(w) => w,
        None => return, // manager not started yet (pre-connect)
    };
    // The roster (fp → pubkey hex) is populated by apply_group_load.
    for peer_hex in state.p2p_group_fp_to_key.values() {
        if peer_hex.is_empty() || *peer_hex == state.profile_public_key {
            continue; // skip self
        }
        // offer_to is idempotent + enforces the offerer rule internally.
        webrtc.offer_to(peer_hex.clone());
    }
}

/// inc-2: push a `{"type":"p2p_group_obj","submission":<submission json>}` frame
/// to every connected roster member over their WebRTC DataChannel. `send_text`
/// only reaches peers whose channel is open — that's correct; the relay POST in
/// the send path covers everyone else. Mirrors web `broadcastGroupObj`.
#[cfg(feature = "native")]
pub(crate) fn broadcast_group_obj(state: &GuiState, group_id: &str, submission_json: &str) {
    // Only broadcast for the group we actually have a roster for (the active one).
    if state.p2p_group_active_id != group_id {
        return;
    }
    let webrtc = match &state.webrtc {
        Some(w) => w,
        None => return,
    };
    // The frame's `submission` is the RAW submission JSON parsed back into a
    // value, so the receiver sees the exact object (not a double-encoded string).
    let submission_val: serde_json::Value = match serde_json::from_str(submission_json) {
        Ok(v) => v,
        Err(_) => return,
    };
    let frame = serde_json::json!({
        "type": "p2p_group_obj",
        "submission": submission_val,
    })
    .to_string();
    for peer_hex in state.p2p_group_fp_to_key.values() {
        if peer_hex.is_empty() || *peer_hex == state.profile_public_key {
            continue;
        }
        webrtc.send_text(peer_hex.clone(), frame.clone());
    }
}

/// inc-2: handle a `p2p_group_obj` frame that arrived over the WebRTC mesh
/// (called from the lib.rs per-frame pump when a `Frame{peer,text}` parses as
/// one). Native mirror of web `handleP2pGroupObj`. Returns true if the frame was
/// a (well-formed) p2p_group_obj we consumed — true even when dropped for a
/// legitimate reason (dup, wrong group, not a member, no key) so the caller does
/// NOT fall back to the inc-1 debug-line behavior; false only if the frame isn't
/// a p2p_group_obj at all.
///
/// SECURITY (the audit-critical path): the submission is UNTRUSTED. We
///   1. require `type == "p2p_group_obj"` with a `submission` object,
///   2. `verify_submission_json` — REJECT unless the ML-DSA signature verifies
///      over the canonical bytes (the same check the relay runs); a forged or
///      tampered object returns None here and is dropped,
///   3. dedup by the LOCALLY-recomputed object_id (never the wire id),
///   4. require object_type == "group_msg_v1" AND references[0] == active group,
///   5. gate author ∈ roster (author_fp must be a key in p2p_group_fp_to_key),
///   6. decrypt under the active epoch key; drop if absent/mismatch (the poll
///      backfills the full multi-epoch history).
/// Only after ALL of these do we render + mark the id seen.
#[cfg(feature = "native")]
pub(crate) fn handle_p2p_group_obj(state: &mut GuiState, _peer: &str, frame_text: &str) -> bool {
    // (0) Is this even a p2p_group_obj frame? If not, signal the caller to keep
    //     its existing behavior (the inc-1 "native p2p test" debug line).
    let frame: serde_json::Value = match serde_json::from_str(frame_text) {
        Ok(v) => v,
        Err(_) => return false,
    };
    if frame.get("type").and_then(|t| t.as_str()) != Some("p2p_group_obj") {
        return false;
    }
    let submission = match frame.get("submission") {
        Some(s) => s,
        None => return true, // malformed p2p_group_obj — consumed (drop)
    };
    let submission_json = submission.to_string();

    // (1+2) Verify the signature. This is the trust boundary — drop on failure.
    let verified = match crate::net::api_v2::verify_submission_json(&submission_json) {
        Some(v) => v,
        None => {
            log::debug!("p2p_group_obj: signature verify FAILED, dropping");
            return true;
        }
    };

    // (4) Only group messages, only for the currently-open group. (Epoch keys
    //     still ride the relay poll; references[0] is the group_id.)
    if verified.object_type != "group_msg_v1" {
        return true;
    }
    if verified.references.first().map(|s| s.as_str()) != Some(state.p2p_group_active_id.as_str())
        || state.p2p_group_active_id.is_empty()
    {
        return true; // for a different/inactive group — ignore
    }

    // (3) Dedup by the locally-recomputed object_id (push or a prior poll).
    if state.p2p_group_seen_obj_ids.contains(&verified.object_id) {
        return true;
    }

    // (5) Membership gate: the author must be in the active roster. fp_to_key's
    //     keys are author fingerprints; its values are the roster pubkey hexes.
    if !state.p2p_group_fp_to_key.contains_key(&verified.author_fp) {
        log::debug!("p2p_group_obj: author not in roster, dropping");
        return true;
    }

    // (6) Decrypt under the active epoch key. If we don't hold the key yet, drop
    //     — the relay poll backfills once the epoch key is fetched. For multi-
    //     epoch correctness the message carries its own epoch; for inc-2 we open
    //     under the active key and drop on mismatch (the poll's full multi-epoch
    //     load reconciles older epochs).
    let epoch_key = match &state.p2p_group_chat_epoch_key {
        Some(k) => k.clone(),
        None => return true,
    };
    let text = match crate::net::group_e2ee::open_group_msg(&verified.payload, &epoch_key) {
        Ok(t) => t,
        Err(e) => {
            // Wrong epoch key (e.g. a re-key the poll hasn't applied) → drop.
            log::debug!("p2p_group_obj: decrypt failed (epoch mismatch?), dropping: {e}");
            return true;
        }
    };

    // Passed every gate — render it. Resolve sender from the roster exactly like
    // replace_p2p_messages does (fp → name / pubkey hex), and use the standard
    // "HH:MM UTC" formatter so the timestamp matches the poll-rendered copy.
    state.p2p_group_seen_obj_ids.insert(verified.object_id.clone());

    let my_key = state.profile_public_key.clone();
    let is_me = verified.author_pubkey_hex == my_key;
    let sender_key = if is_me {
        my_key.clone()
    } else {
        state
            .p2p_group_fp_to_key
            .get(&verified.author_fp)
            .cloned()
            .unwrap_or_else(|| verified.author_pubkey_hex.clone())
    };
    let sender_name = if is_me {
        if !state.user_name.is_empty() { state.user_name.clone() } else { "You".to_string() }
    } else {
        state
            .p2p_group_fp_to_name
            .get(&verified.author_fp)
            .cloned()
            .unwrap_or_else(|| {
                format!("{}…", &verified.author_fp[..12.min(verified.author_fp.len())])
            })
    };
    let created_ms = verified.created_at.max(0) as u64;
    let channel = format!("p2pgroup:{}", state.p2p_group_active_id);
    state.chat_messages.push(ChatMessage {
        sender_name,
        sender_key,
        content: text,
        timestamp: format_timestamp(created_ms),
        timestamp_ms: created_ms,
        channel,
        ..Default::default()
    });
    state.chat_messages.sort_by_key(|m| m.timestamp_ms);
    while state.chat_messages.len() > 400 {
        state.chat_messages.remove(0);
    }
    true
}

// ─────────────────────────────── UI Helpers ──────────────────────────────

/// Draw a lock/unlock toggle button — designed to sit flush in a panel
/// corner with NO surrounding padding. Allocates exactly 14×14 with no
/// horizontal wrapper or item spacing, so when the caller positions an
/// Area at the panel boundary the button paints exactly there.
/// Returns true if the button was clicked (toggle lock state).
fn draw_panel_lock_button(ui: &mut egui::Ui, _theme: &Theme, locked: bool) -> bool {
    let tooltip = if locked { "Unlock panel width" } else { "Lock panel width" };
    let color = if locked { Color32::from_rgb(200, 180, 100) } else { Color32::from_rgb(100, 100, 100) };
    let (rect, resp) = crate::gui::widgets::icons::icon_button(ui, 14.0);
    if locked {
        crate::gui::widgets::icons::paint_lock(ui.painter(), rect, color);
    } else {
        crate::gui::widgets::icons::paint_unlock(ui.painter(), rect, color);
    }
    resp.on_hover_text(tooltip).clicked()
}

/// Draw a collapsible section header with a tinted background.
/// Returns true if the header was clicked (toggle).
fn tinted_section_header(ui: &mut egui::Ui, label: &str, collapsed: bool, bg: Color32) -> bool {
    tinted_section_header_with_buttons(ui, label, collapsed, bg, |_| {})
}

/// Draw a tinted section header with optional right-aligned buttons.
/// The `add_buttons` closure receives the UI in right-to-left layout for adding icon buttons.
/// Returns true if the collapse arrow area was clicked (toggle collapse).
fn tinted_section_header_with_buttons(
    ui: &mut egui::Ui,
    label: &str,
    collapsed: bool,
    bg: Color32,
    add_buttons: impl FnOnce(&mut egui::Ui),
) -> bool {
    let header_height = 28.0;
    let full_width = ui.available_width();

    // Allocate the full header rect for background painting
    let (full_rect, _) = ui.allocate_exact_size(
        Vec2::new(full_width, header_height),
        egui::Sense::hover(),
    );

    // Paint background
    let header_bg = Color32::from_rgba_premultiplied(
        bg.r().saturating_add(15),
        bg.g().saturating_add(15),
        bg.b().saturating_add(15),
        bg.a(),
    );
    ui.painter().rect_filled(full_rect, 0.0, header_bg);

    // Place the collapse arrow as a separate click target
    let arrow_size = 12.0;
    let arrow_left = full_rect.left() + 8.0;
    let cy = full_rect.center().y;
    let arrow_click_rect = egui::Rect::from_min_size(
        egui::pos2(full_rect.left(), full_rect.top()),
        Vec2::new(arrow_size + 16.0, header_height), // wider click target for the collapse area
    );
    let arrow_resp = ui.interact(arrow_click_rect, ui.id().with(label).with("arrow"), egui::Sense::click());

    // Draw the arrow
    let arrow_rect = egui::Rect::from_min_size(
        egui::pos2(arrow_left, cy - arrow_size / 2.0),
        Vec2::splat(arrow_size),
    );
    let arrow_color = Color32::from_rgb(180, 180, 180);
    if collapsed {
        crate::gui::widgets::icons::paint_triangle_right(ui.painter(), arrow_rect, arrow_color);
    } else {
        crate::gui::widgets::icons::paint_triangle_down(ui.painter(), arrow_rect, arrow_color);
    }

    // Draw the label
    let label_x = arrow_left + arrow_size + 4.0;
    ui.painter().text(
        egui::pos2(label_x, cy),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.0),
        Color32::from_rgb(200, 200, 200),
    );

    // Draw right-aligned buttons using a child UI positioned at the right side
    let btn_rect = egui::Rect::from_min_max(
        egui::pos2(full_rect.right() - 80.0, full_rect.top()),
        full_rect.max,
    );
    let mut btn_ui = ui.new_child(egui::UiBuilder::new().max_rect(btn_rect).layout(Layout::right_to_left(Align::Center)));
    btn_ui.add_space(4.0);
    add_buttons(&mut btn_ui);

    if arrow_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    arrow_resp.clicked()
}

/// Draw a plain collapsible section header (for right panel).
fn section_header(ui: &mut egui::Ui, label: &str, collapsed: bool, bg: Color32) -> bool {
    let response = ui
        .allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 32.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                let full_rect = ui.max_rect();
                ui.painter().rect_filled(full_rect, 0.0, bg);
                ui.add_space(10.0);

                // Painted collapse/expand triangle
                let arrow_color = Color32::from_rgb(180, 180, 180);
                let (arrow_rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
                if collapsed {
                    crate::gui::widgets::icons::paint_triangle_right(ui.painter(), arrow_rect, arrow_color);
                } else {
                    crate::gui::widgets::icons::paint_triangle_down(ui.painter(), arrow_rect, arrow_color);
                }
                ui.label(
                    RichText::new(label)
                        .size(13.0)
                        .color(Color32::from_rgb(200, 200, 200))
                        .strong(),
                );
            },
        )
        .response;

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

/// Draw colored role badge pills (A=admin red, M=mod orange, V=verified blue).
fn draw_role_badges(
    ui: &mut egui::Ui,
    theme: &Theme,
    role: &str,
    roles: &[crate::relay::storage::RoleDef],
) {
    if role.is_empty() || role == "member" || role == "unverified" {
        return;
    }

    // Prefer the data-driven RoleDef (so CUSTOM roles get a badge too,
    // in their own color). Fall back to the legacy hardcoded theme-token
    // badges when the role_list hasn't arrived yet OR the role isn't in
    // it (keeps behavior identical pre-role_list / for legacy ids).
    // Roles Phase R2, v0.241; see docs/design/roles-system.md.
    let (badge_char, badge_color): (String, Color32) =
        if let Some(rd) = roles.iter().find(|r| r.id == role) {
            // Built-ins keep their familiar single letter (A/M/V/D) for
            // continuity; custom roles use the first letter of the label.
            let ch = match rd.id.as_str() {
                "admin" => "A".to_string(),
                "mod" => "M".to_string(),
                "verified" => "V".to_string(),
                "donor" => "D".to_string(),
                _ => rd.label.chars().next()
                        .map(|c| c.to_uppercase().to_string())
                        .unwrap_or_else(|| "?".to_string()),
            };
            (ch, parse_role_color(&rd.color, theme))
        } else {
            match role {
                "admin" => ("A".to_string(), Theme::c32(&theme.badge_admin)),
                "moderator" | "mod" => ("M".to_string(), Theme::c32(&theme.badge_mod)),
                "verified" => ("V".to_string(), Theme::c32(&theme.badge_verified)),
                "donor" => ("D".to_string(), Theme::c32(&theme.badge_donor)),
                _ => return,
            }
        };

    let text = RichText::new(&badge_char)
        .size(theme.font_size_small - 2.0)
        .color(Color32::WHITE)
        .strong();

    let galley = ui.fonts(|f| f.layout_no_wrap(badge_char.clone(), egui::FontId::proportional(theme.font_size_small - 2.0), Color32::WHITE));
    let badge_width = galley.size().x + 8.0;
    let badge_height = 16.0;

    let (rect, _) = ui.allocate_exact_size(Vec2::new(badge_width, badge_height), egui::Sense::hover());
    ui.painter().rect_filled(rect, 3.0, badge_color);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        badge_char,
        egui::FontId::proportional(theme.font_size_small - 2.0),
        Color32::WHITE,
    );
    let _ = text; // suppress unused warning
}

/// Get the viewer's own role by matching their public key in the user list.
fn viewer_role(state: &GuiState) -> String {
    if state.profile_public_key.is_empty() {
        return String::new();
    }
    state.chat_users.iter()
        .find(|u| u.public_key == state.profile_public_key)
        .map(|u| u.role.clone())
        .unwrap_or_default()
}

/// Extract a display name from a server URL.
fn server_display_name(url: &str) -> String {
    let cleaned = url
        .replace("https://", "")
        .replace("http://", "")
        .replace("wss://", "")
        .replace("ws://", "")
        .trim_end_matches('/')
        .trim_end_matches("/ws")
        .to_string();
    if cleaned.is_empty() {
        "Server".to_string()
    } else {
        cleaned
    }
}

/// Truncate a string to max chars, adding "..." if truncated.
fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

// ─────────────────────────────── Shared Helpers ──────────────────────────

/// Measure the exact width the timestamp pill needs. Used by chat.rs to
/// pass `pill_width` into message_row so the row reserves the correct
/// amount of horizontal space. paint_timestamp_pill MUST keep its
/// rendering math in sync with this function — if they diverge the
/// reservation will be wrong and message text will overlap the pill.
///
/// Layout (matches paint_timestamp_pill):
///   [ 6px pad | timestamp | 5px gap | Þ | 4px gap | badges (3px gap each) | 6px pad ]
fn compute_pill_width(
    ctx: &egui::Context,
    theme: &Theme,
    timestamp: &str,
    reactions: &std::collections::HashMap<String, Vec<String>>,
) -> f32 {
    let ts_clean = timestamp.trim().trim_end_matches(" UTC").trim().to_string();
    let ts_w = ctx.fonts(|f| {
        f.layout_no_wrap(
            ts_clean,
            egui::FontId::proportional(theme.small_size),
            theme.text_muted(),
        )
    }).size().x;

    let thorn_w = ctx.fonts(|f| {
        f.layout_no_wrap(
            "Þ".to_string(),
            egui::FontId::proportional(theme.font_size_body),
            theme.accent(),
        )
    }).size().x;

    // Base layout (must match paint_timestamp_pill):
    //   left_pad(6) + ts_w + gap(5) + thorn_w + right_pad(6)
    let mut total = 6.0 + ts_w + 5.0 + thorn_w + 6.0;

    if !reactions.is_empty() {
        // When badges are present they REPLACE the right pad with:
        // gap_before_first(4) + sum(badge_body) + sum(gap 3 between)
        // + right_pad(6).
        total -= 6.0;
        let mut first = true;
        for (emoji, keys) in {
            let mut e: Vec<(&String, &Vec<String>)> = reactions.iter().collect();
            e.sort_by(|a, b| a.0.cmp(b.0));
            e.into_iter().take(4)
        } {
            if keys.is_empty() { continue; }
            // Match paint_timestamp_pill — strip FE0F before measuring so
            // the badge width estimate matches the rendered width.
            let display_emoji: String = emoji.chars().filter(|c| *c != '\u{FE0F}').collect();
            let label = format!("{}{}", display_emoji, keys.len());
            let label_w = ctx.fonts(|f| {
                f.layout_no_wrap(
                    label,
                    egui::FontId::proportional(theme.small_size),
                    theme.text_primary(),
                )
            }).size().x;
            total += if first { 4.0 } else { 3.0 };
            first = false;
            total += label_w + 4.0; // badge body = label_w + 2px each side
        }
        total += 6.0; // right pad after last badge
    }
    total.ceil()
}

/// Paint the inline timestamp pill: a small rounded frame containing the
/// timestamp text, a Þ separator, and any existing reaction badges (with
/// counts). Anchored at the rect message_row reserved.
///
/// Clicking an existing reaction toggles your own. Clicking Þ has no
/// dedicated action (the pill expands via row hover; see the
/// `pill_expand` Area in the message render block).
/// Returns the rect occupied by the Þ pull-tab marker so the caller can
/// constrain reaction-pill popup hover detection to JUST the Þ (instead
/// of the entire timestamp pill, which would cover the message text).
/// Operator feedback 2026-05-12 — "I only want the reaction pill to come
/// up if I mouse over the Þ icon or click on it. Right now if I mouse
/// over reply text the reaction pill comes up which prevents me from
/// interacting with text (like copying it to paste or googling a key
/// word.)"
fn paint_timestamp_pill(
    ui: &mut egui::Ui,
    theme: &Theme,
    rect: egui::Rect,
    timestamp: &str,
    reactions: &std::collections::HashMap<String, Vec<String>>,
    my_key: &str,
    msg_ts_ms: u64,
    msg_sender_key: String,
    pending_reactions: &mut Vec<(String, u64, String)>,
    // When true (popup is open for this message), paint the Þ in the
    // channeling RGB cycle instead of static theme.accent to signal
    // "active" — matches the nav-border / escape-menu animated feedback
    // used elsewhere (operator feedback 2026-05-12).
    popup_active: bool,
) -> egui::Rect {
    let painter = ui.painter();
    // Pill background — fully OPAQUE so the underlying transparent layout
    // spacer doesn't let message text bleed through. Earlier the alpha
    // was 200 which produced visible text overlap on long pill widths.
    painter.rect_filled(rect, theme.pill_radius, theme.bg_card());
    painter.rect_stroke(
        rect,
        theme.pill_radius,
        Stroke::new(1.0, theme.border()),
        egui::StrokeKind::Inside,
    );

    let ts_clean = timestamp.trim().trim_end_matches(" UTC").trim();
    let cy = rect.center().y;
    let mut x = rect.left() + 6.0; // left pad — must match compute_pill_width

    // Timestamp text
    let ts_galley = ui.fonts(|f| {
        f.layout_no_wrap(
            ts_clean.to_string(),
            egui::FontId::proportional(theme.small_size),
            theme.text_muted(),
        )
    });
    let ts_h = ts_galley.size().y;
    let ts_w = ts_galley.size().x;
    painter.galley(egui::pos2(x, cy - ts_h / 2.0), ts_galley, theme.text_muted());
    x += ts_w + 5.0; // ts width + gap before Þ — must match compute

    // Þ pull-tab marker. When popup_active is true the glyph paints in
    // the channeling-RGB cycle (matches escape_menu nav-border and other
    // animated UI feedback) so the user can see the Þ is "live" — exits
    // back to static accent once they hover off both Þ and popup.
    // ChannelingColor depends on ui.ctx().input(|i| i.time) so a repaint
    // is requested in the calling block.
    let thorn_color = if popup_active {
        let chan_time = ui.ctx().input(|i| i.time) as f32;
        crate::gui::pages::escape_menu::channeling_color(
            theme,
            chan_time,
            false, // chan_attack flag unused here
            theme.accent(),
        )
    } else {
        theme.accent()
    };
    let thorn_galley = ui.fonts(|f| {
        f.layout_no_wrap(
            "Þ".to_string(),
            egui::FontId::proportional(theme.font_size_body),
            thorn_color,
        )
    });
    let thorn_h = thorn_galley.size().y;
    let thorn_w = thorn_galley.size().x;
    // Pad the Þ hit-rect by ~2 px on each side so the hover area isn't
    // pixel-tight (otherwise small cursor jitter dismisses the popup
    // before the user can slide into it).
    let thorn_hit = egui::Rect::from_min_size(
        egui::pos2(x - 2.0, cy - thorn_h / 2.0 - 2.0),
        Vec2::new(thorn_w + 4.0, thorn_h + 4.0),
    );
    painter.galley(egui::pos2(x, cy - thorn_h / 2.0), thorn_galley, thorn_color);
    if popup_active {
        // Animation needs a continuous repaint loop while active.
        ui.ctx().request_repaint();
    }
    x += thorn_w; // advance past Þ; first-badge gap added below

    // Existing reaction badges (right of Þ). Each = [2px-pad emoji+count 2px-pad]
    // followed by 3px gap to the next badge. First badge follows Þ after a 4px gap.
    if !reactions.is_empty() {
        let mut emojis: Vec<(&String, &Vec<String>)> = reactions.iter().collect();
        emojis.sort_by(|a, b| a.0.cmp(b.0));
        let mut first = true;
        for (emoji, keys) in emojis.into_iter().take(4) {
            let count = keys.len();
            if count == 0 { continue; }
            // Strip U+FE0F variation selector from any pre-existing reaction
            // (older clients may have stored "❤️" with the selector — render
            // path now uses bare codepoint to avoid the trailing tofu square).
            let display_emoji: String = emoji.chars().filter(|c| *c != '\u{FE0F}').collect();
            let i_reacted = keys.contains(&my_key.to_string());
            let label = format!("{}{}", display_emoji, count);
            let label_galley = ui.fonts(|f| {
                f.layout_no_wrap(
                    label.clone(),
                    egui::FontId::proportional(theme.small_size),
                    if i_reacted { theme.accent() } else { theme.text_primary() },
                )
            });
            let label_w = label_galley.size().x;
            let badge_w = label_w + 4.0; // 2px internal pad each side
            // Pre-badge gap: 4 for first, 3 for subsequent — must match compute.
            x += if first { 4.0 } else { 3.0 };
            first = false;
            let badge_rect = egui::Rect::from_min_size(
                egui::pos2(x, cy - 8.0),
                Vec2::new(badge_w, 16.0),
            );
            // Stop drawing if we'd overflow the reserved pill width.
            if badge_rect.right() > rect.right() - 2.0 { break; }
            let badge_bg = if i_reacted {
                let a = theme.accent();
                Color32::from_rgba_unmultiplied(a.r(), a.g(), a.b(), 60)
            } else {
                Color32::TRANSPARENT
            };
            painter.rect_filled(badge_rect, Rounding::same(7), badge_bg);
            painter.galley(
                egui::pos2(badge_rect.left() + 2.0, cy - label_galley.size().y / 2.0),
                label_galley,
                if i_reacted { theme.accent() } else { theme.text_primary() },
            );
            // Click badge to toggle this reaction. Send the SAME key that was
            // stored (with or without FE0F) so the relay matches and toggles
            // the correct entry — don't substitute the cleaned display string.
            let resp = ui.interact(
                badge_rect,
                egui::Id::new(("react_pill_inline", msg_ts_ms, emoji.clone())),
                egui::Sense::click(),
            );
            if resp.clicked() {
                pending_reactions.push((msg_sender_key.clone(), msg_ts_ms, emoji.clone()));
            }
            x += badge_w; // advance past the badge body; next-badge gap added next iter
        }
    }
    thorn_hit
}

/// Format a UNIX millisecond timestamp as "YYYY-MM-DD HH:MM:SS UTC".
/// Uses Howard Hinnant's days-from-civil algorithm so we don't need
/// chrono as a dependency. Accurate for any proleptic-Gregorian year.
///
/// Used by the Þ-hover timestamp-expansion overlay in the message
/// pill (operator feedback 2026-05-12 — "when we interact with the
/// timestamp ... on the left the timestamp expands to include the
/// YEAR:MONTH:DAY:HOUR:MINUTE:SECOND").
/// Always-full timestamp (the pill-expand hover popup), independent of the
/// user's display-format setting — the popup is the "show me everything" view.
pub fn format_full_timestamp(ts_ms: u64) -> String {
    let (year, month, day, hour, minute, second, _ms) = ts_parts(ts_ms);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        year, month, day, hour, minute, second
    )
}

/// Where to file a server-level system / private-server notice (relay messages,
/// deploy-bot announcements, command responses). These were tagged with the
/// *active* channel — but when the user is viewing a private P2P context (a
/// `p2pgroup:` group or a `dm:` conversation), that leaks the notice INTO the
/// private view, where it then vanishes on the next group reload (which rebuilds
/// strictly from the group's signed message log). So when the active channel is
/// private, file the notice under "general" (a server channel) instead — it's
/// preserved without polluting the private conversation. (Bug: a deploy-bot
/// #announcements notice appeared inside an open P2P group, then disappeared.)
pub fn notice_channel(active_channel: &str) -> String {
    if active_channel.starts_with("p2pgroup:") || active_channel.starts_with("dm:") {
        "general".to_string()
    } else {
        active_channel.to_string()
    }
}

/// Try to read an image from the OS clipboard and encode it as PNG bytes.
/// Returns `None` when the clipboard has no image (e.g. just text), or when
/// the clipboard / encoder errors. The native arboard crate handles
/// Windows / macOS / Linux clipboard access. v0.232.
fn try_grab_clipboard_image_as_png() -> Option<Vec<u8>> {
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => { log::warn!("Clipboard open failed: {e}"); return None; }
    };
    let img = match clipboard.get_image() {
        Ok(img) => img,
        Err(arboard::Error::ContentNotAvailable) => return None, // text-only clipboard
        Err(e) => { log::warn!("Clipboard get_image failed: {e}"); return None; }
    };
    // arboard returns RGBA8 bytes in `img.bytes` with dimensions in
    // `img.width` / `img.height`. Re-encode as PNG via the `image` crate
    // (already a dep) before uploading.
    let width = img.width as u32;
    let height = img.height as u32;
    let buf = image::RgbaImage::from_raw(width, height, img.bytes.into_owned())?;
    let mut png_bytes: Vec<u8> = Vec::new();
    if let Err(e) = image::DynamicImage::ImageRgba8(buf)
        .write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
    {
        log::warn!("PNG encode of clipboard image failed: {e}");
        return None;
    }
    Some(png_bytes)
}

/// Blocking upload of a PNG to `<server_url>/api/upload?key=<pk>` as a
/// multipart/form-data body. Returns the resulting URL (parsed from the
/// JSON response `{"url":"...","filename":"...","size":N,"type":"..."}`).
/// Mirrors the web client's `uploadImage(file)` flow.
///
/// Synchronous (blocks the egui draw frame for the duration of the upload).
/// For typical print-screen captures (~200-800 KB after PNG encoding) on a
/// decent network the upload completes in well under a second. If this
/// becomes annoying we can move it to a background tokio task; the simple
/// blocking version is the right starting point. v0.232.
fn upload_image_png_blocking(
    server_url: &str,
    public_key: &str,
    png_bytes: Vec<u8>,
) -> Result<String, String> {
    let base = server_url.trim_end_matches('/');
    // public_key is a hex string (64 chars), no URL-encoding needed.
    let upload_url = format!("{base}/api/upload?key={key}",
        base = base, key = public_key);
    // Construct a multipart/form-data body manually — ureq 2 doesn't
    // include multipart helpers and we don't want to pull in a crate
    // just for this. Header: `Content-Type: multipart/form-data; boundary=<b>`.
    let boundary = format!("HumanityOSBoundary{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let preamble = format!(
        "--{b}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"clipboard.png\"\r\n\
         Content-Type: image/png\r\n\r\n",
        b = boundary,
    );
    let epilogue = format!("\r\n--{b}--\r\n", b = boundary);
    let mut body: Vec<u8> = Vec::with_capacity(preamble.len() + png_bytes.len() + epilogue.len());
    body.extend_from_slice(preamble.as_bytes());
    body.extend_from_slice(&png_bytes);
    body.extend_from_slice(epilogue.as_bytes());

    let resp = ureq::post(&upload_url)
        .set("Content-Type", &format!("multipart/form-data; boundary={}", boundary))
        .send_bytes(&body)
        .map_err(|e| format!("HTTP POST failed: {e}"))?;
    let body_str = resp.into_string()
        .map_err(|e| format!("read response: {e}"))?;
    let val: serde_json::Value = serde_json::from_str(&body_str)
        .map_err(|e| format!("parse JSON: {e}; body={body_str}"))?;
    val.get("url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("response missing 'url' field: {body_str}"))
}

/// Convert an HTTPS URL to a WSS URL for the relay.
pub fn derive_ws_url(url: &str) -> String {
    let base = url
        .trim_end_matches('/')
        .replace("https://", "wss://")
        .replace("http://", "ws://");
    if base.ends_with("/ws") {
        base
    } else {
        format!("{}/ws", base)
    }
}

/// Generate a random 64-char hex string to use as a placeholder public key.
pub fn generate_random_hex_key() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Derive a consistent color from a username (for avatar circles).
/// Parse a role's "#RRGGBB" badge color into a Color32. Falls back to
/// the theme's primary text color if the string isn't a valid 6-digit
/// hex (so a malformed custom-role color can't make the label invisible).
/// v0.241 (roles Phase R2).
fn parse_role_color(hex: &str, theme: &Theme) -> Color32 {
    let h = hex.trim().trim_start_matches('#');
    if h.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&h[0..2], 16),
            u8::from_str_radix(&h[2..4], 16),
            u8::from_str_radix(&h[4..6], 16),
        ) {
            return Color32::from_rgb(r, g, b); // theme-exempt: data-driven role color from server
        }
    }
    theme.text_primary()
}

fn name_color(name: &str) -> Color32 {
    let hash: u32 = name.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    let hue = (hash % 360) as f32;
    let s = 0.5_f32;
    let l = 0.45_f32;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = if hue < 60.0 {
        (c, x, 0.0)
    } else if hue < 120.0 {
        (x, c, 0.0)
    } else if hue < 180.0 {
        (0.0, c, x)
    } else if hue < 240.0 {
        (0.0, x, c)
    } else if hue < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    Color32::from_rgb(
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

/// Return a human-readable timestamp string for "now".
fn chrono_now_str() -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    // Route through the setting-aware formatter so a local send-echo matches the
    // user's chosen timestamp format (and the relay-reconciled copy).
    format_timestamp(now_ms)
}

/// Format a Unix-millis timestamp to HH:MM UTC.
/// User-selectable timestamp display granularity (operator request). All UTC.
/// Drives `format_timestamp` app-wide via a process-global so the pure formatter
/// doesn't need GuiState threaded through every call site.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimestampFormat {
    HourMin,        // 17:42 UTC   (default)
    HourMinSec,     // 17:42:09 UTC
    DateHourMin,    // 2026-05-29 17:42 UTC
    DateHourMinSec, // 2026-05-29 17:42:09 UTC
    Full,           // 2026-05-29 17:42:09.123 UTC
}
impl TimestampFormat {
    /// Stable string for config persistence.
    pub fn as_str(self) -> &'static str {
        match self {
            TimestampFormat::HourMin => "hour_min",
            TimestampFormat::HourMinSec => "hour_min_sec",
            TimestampFormat::DateHourMin => "date_hour_min",
            TimestampFormat::DateHourMinSec => "date_hour_min_sec",
            TimestampFormat::Full => "full",
        }
    }
    pub fn from_config_str(s: &str) -> Self {
        match s {
            "hour_min_sec" => TimestampFormat::HourMinSec,
            "date_hour_min" => TimestampFormat::DateHourMin,
            "date_hour_min_sec" => TimestampFormat::DateHourMinSec,
            "full" => TimestampFormat::Full,
            _ => TimestampFormat::HourMin,
        }
    }
    /// Human label for the settings dropdown (with a live example).
    pub fn label(self) -> &'static str {
        match self {
            TimestampFormat::HourMin => "Time, 17:42",
            TimestampFormat::HourMinSec => "Time + seconds, 17:42:09",
            TimestampFormat::DateHourMin => "Date + time, 2026-05-29 17:42",
            TimestampFormat::DateHourMinSec => "Date + time + seconds, 2026-05-29 17:42:09",
            TimestampFormat::Full => "Full + milliseconds, 2026-05-29 17:42:09.123",
        }
    }
    pub const ALL: [TimestampFormat; 5] = [
        TimestampFormat::HourMin,
        TimestampFormat::HourMinSec,
        TimestampFormat::DateHourMin,
        TimestampFormat::DateHourMinSec,
        TimestampFormat::Full,
    ];
    fn discriminant(self) -> usize {
        TimestampFormat::ALL.iter().position(|&f| f == self).unwrap_or(0)
    }
    fn from_discriminant(d: usize) -> Self {
        TimestampFormat::ALL.get(d).copied().unwrap_or(TimestampFormat::HourMin)
    }
}

static TS_FORMAT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// Set the app-wide timestamp display format (call on config load + settings change).
pub fn set_timestamp_format(f: TimestampFormat) {
    TS_FORMAT.store(f.discriminant(), std::sync::atomic::Ordering::Relaxed);
}
/// The current app-wide timestamp display format.
pub fn timestamp_format() -> TimestampFormat {
    TimestampFormat::from_discriminant(TS_FORMAT.load(std::sync::atomic::Ordering::Relaxed))
}

/// Break an epoch-ms timestamp into UTC (year, month, day, hour, minute, second, millis).
/// Howard Hinnant days-from-civil → calendar date (no chrono dependency).
fn ts_parts(ts_ms: u64) -> (i32, u32, u32, i64, i64, i64, u64) {
    let unix_s = (ts_ms / 1000) as i64;
    let millis = ts_ms % 1000;
    let days = unix_s.div_euclid(86_400);
    let secs_in_day = unix_s.rem_euclid(86_400);
    let hour = secs_in_day / 3_600;
    let minute = (secs_in_day % 3_600) / 60;
    let second = secs_in_day % 60;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { (mp + 3) as u32 } else { (mp - 9) as u32 };
    let year = if month <= 2 { (y + 1) as i32 } else { y as i32 };
    (year, month, day, hour, minute, second, millis)
}

/// Format an epoch-ms timestamp per the user's chosen `timestamp_format()` —
/// used for the message-pill across the whole chat, so the setting applies
/// everywhere at once. Always UTC.
pub fn format_timestamp(ts: u64) -> String {
    let (y, mo, d, h, mi, s, ms) = ts_parts(ts);
    match timestamp_format() {
        TimestampFormat::HourMin => format!("{:02}:{:02} UTC", h, mi),
        TimestampFormat::HourMinSec => format!("{:02}:{:02}:{:02} UTC", h, mi, s),
        TimestampFormat::DateHourMin => format!("{:04}-{:02}-{:02} {:02}:{:02} UTC", y, mo, d, h, mi),
        TimestampFormat::DateHourMinSec => {
            format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC", y, mo, d, h, mi, s)
        }
        TimestampFormat::Full => {
            format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03} UTC", y, mo, d, h, mi, s, ms)
        }
    }
}

/// Send a slash command as a chat message (server handles moderation via slash commands).
fn send_slash_command(state: &mut GuiState, command: &str) {
    if let Some(ref client) = state.ws_client {
        if client.is_connected() {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let mut msg = serde_json::json!({
                "type": "chat",
                "from": state.profile_public_key,
                "from_name": state.user_name,
                "content": command,
                "timestamp": ts,
                "channel": state.chat_active_channel,
            });
            // Inc2.MED-1: Dilithium chat signature.
            if let Some(seed) = state.private_key_bytes.as_ref() {
                msg["pq_signature"] = serde_json::Value::String(
                    crate::net::identity::pq_sign_chat(seed, command, ts)
                );
            }
            client.send(&msg.to_string());
        }
    }
}

// ─────────────────────────────── Help Modal ──────────────────────────────

fn draw_help_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let mut open = state.show_help_modal;
    widgets::dialog(ctx, theme, "slash_commands_dialog", "Slash Commands", &mut open, |ui| {
        ui.set_min_width(420.0);
        ScrollArea::vertical()
            .id_salt("help_modal_scroll")
            .auto_shrink([false, false])
            .max_height(440.0)
            .show(ui, |ui| {
                    let section_color = Color32::from_rgb(100, 180, 255);
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
                        ("/dm <name> <message>", "Send a direct message"),
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
                    ui.label(RichText::new("Moderator").size(theme.font_size_body + 2.0).color(Color32::from_rgb(255, 180, 80)).strong());
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
                    ui.label(RichText::new("Admin").size(theme.font_size_body + 2.0).color(Color32::from_rgb(255, 100, 100)).strong());
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
                    ui.label(RichText::new("Federation").size(theme.font_size_body + 2.0).color(Color32::from_rgb(100, 220, 160)).strong());
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
                    ui.label(RichText::new("Click the reply arrow on any message to reply").size(theme.font_size_body).color(desc_color));
                });
    });
    if !open {
        state.show_help_modal = false;
    }
}

// ──────────────────────────────────────────────────────────────────────────
// B3 fix (v0.199.0): DM crypto silent downgrade prevention.
// ──────────────────────────────────────────────────────────────────────────

/// Attempt to encrypt a DM. Returns `Ok((content_b64, nonce_b64))` if
/// encryption succeeds, `Err(reason)` if it can't be encrypted at all.
/// The reason string flows into `PendingUnencryptedDm::reason` for the
/// confirmation modal.
///
/// Full-PQ: dual-seals (recipient + self) with Kyber768 and returns
/// `(envelope_json, nonce_marker)`. The envelope is the web-compatible
/// `{v:1,r,s}` JSON that rides in the relay's opaque `content` field;
/// the top-level wire `nonce` is vestigial (the authoritative nonces are
/// inside the envelope — kept non-empty only so any legacy
/// `encrypted && nonce` predicate still trips).
///
/// Failure reasons:
///   - `"no_own_key"`        — the BIP39 seed isn't unlocked on this device
///   - `"missing_peer_key"`  — recipient's Kyber768 public key isn't known
///   - `"bad_own_key"`       — Kyber keypair derivation failed
///   - `"encryption_failed"` — seal_envelope() returned an error
fn try_encrypt_dm(
    state: &GuiState,
    partner_key: &str,
    content: &str,
) -> Result<(String, String), &'static str> {
    let seed = state.private_key_bytes.as_ref().ok_or("no_own_key")?;
    let my_kp = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(seed)
        .map_err(|_| "bad_own_key")?;
    let peer_kyber = state.peer_kyber_keys.get(partner_key)
        .ok_or("missing_peer_key")?;
    let envelope = crate::net::dm_pq::seal_envelope(
        peer_kyber, &my_kp.public_base64(), content,
    )
    .map_err(|_| "encryption_failed")?;
    Ok((envelope, "pq".to_string()))
}

/// Render the unencrypted-DM confirmation modal if one is pending.
/// Pops up when the user clicked Send on a DM that we couldn't encrypt
/// (B3 fix). User must explicitly confirm "Send unencrypted" or cancel
/// — no silent plaintext fallback. Call from chat::draw.
pub(crate) fn draw_unencrypted_dm_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let pending = match state.dm_unencrypted_confirm.clone() {
        Some(p) => p,
        None => return,
    };

    let reason_human = match pending.reason.as_str() {
        "no_own_key" => "Your identity isn't unlocked on this device, recover from your seed phrase to send encrypted DMs.",
        "missing_peer_key" => "We don't have the recipient's post-quantum key yet, they may not have come online with a current client, or their key broadcast hasn't reached us.",
        "bad_own_key" =>
            "Your post-quantum key could not be derived on this device. Try Identity → Recover.",
        "encryption_failed" => "Encryption failed unexpectedly.",
        other => other,
    };

    let mut close_modal = false;
    let mut send_anyway = false;
    let mut cancel_clicked = false;

    egui::Window::new("⚠ Unencrypted DM Confirmation")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(egui::Vec2::new(440.0, 0.0))
        .frame(egui::Frame::window(&ctx.style()).fill(theme.bg_card()))
        .show(ctx, |ui| {
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new(format!("Sending to: {}", pending.partner_name))
                    .size(theme.font_size_body)
                    .color(theme.text_primary())
                    .strong(),
            );
            ui.add_space(theme.spacing_xs);
            ui.label(
                RichText::new("This message CANNOT be end-to-end encrypted.")
                    .size(theme.font_size_body)
                    .color(theme.danger())
                    .strong(),
            );
            ui.add_space(theme.spacing_xs);
            ui.label(
                RichText::new(reason_human)
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new("Sending unencrypted means anyone with access to the relay server can read it. The recipient is NOT protected from a hostile or compromised relay. Continue?")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_sm);
            ui.separator();
            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new("Your message:")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            // Show a preview of the message body, truncated to keep the
            // modal compact even for long drafts.
            let preview = if pending.content.chars().count() > 240 {
                let truncated: String = pending.content.chars().take(240).collect();
                format!("{}…", truncated)
            } else {
                pending.content.clone()
            };
            egui::Frame::none()
                .fill(theme.bg_panel())
                .rounding(egui::Rounding::same(theme.border_radius as u8))
                .inner_margin(theme.card_padding)
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(&preview)
                            .size(theme.font_size_small)
                            .color(theme.text_primary())
                            .monospace(),
                    );
                });
            ui.add_space(theme.spacing_md);
            ui.horizontal(|ui| {
                if widgets::Button::secondary("Cancel, keep it private")
                    .tooltip("Don't send. The message is restored to your input box so you can wait for the recipient's encryption key, edit, or copy it elsewhere.")
                    .show(ui, theme)
                {
                    cancel_clicked = true;
                    close_modal = true;
                }
                ui.add_space(theme.spacing_sm);
                if widgets::Button::danger("Send unencrypted anyway")
                    .tooltip("Send the plaintext message NOW. The relay (and anyone with access to it) will be able to read it.")
                    .show(ui, theme)
                {
                    send_anyway = true;
                    close_modal = true;
                }
            });
            ui.add_space(theme.spacing_sm);
        });

    if cancel_clicked {
        // Restore draft into the input box so the user can decide what to do.
        state.chat_input = pending.content.clone();
    }

    if send_anyway {
        // Build and send the plaintext DM directly. We bypass the normal
        // send path because that path would re-trigger the confirm modal.
        if let Some(ref client) = state.ws_client {
            if client.is_connected() {
                let display_name = if !state.user_name.is_empty() {
                    state.user_name.clone()
                } else if let Some(me) = state.chat_users.iter().find(|u| u.public_key == state.profile_public_key) {
                    if !me.name.is_empty() && me.name != "Anonymous" { me.name.clone() } else { "Anonymous".to_string() }
                } else {
                    "Anonymous".to_string()
                };
                let dm_obj = serde_json::json!({
                    "type": "dm",
                    "from": state.profile_public_key,
                    "from_name": display_name,
                    "to": pending.partner_key,
                    "content": pending.content,
                    "timestamp": pending.timestamp_ms,
                    // Explicit false so the relay + recipient can show
                    // an "unencrypted" indicator if they want.
                    "encrypted": false,
                });
                let json_str = dm_obj.to_string();
                crate::debug::push_debug(format!("WS >>> [user-confirmed plaintext] {}", json_str));
                client.send(&json_str);
                state.chat_sent_timestamps.push(pending.timestamp_ms);
                if state.chat_sent_timestamps.len() > 20 {
                    state.chat_sent_timestamps.remove(0);
                }
                // Local echo so the user sees their own message immediately,
                // matching the normal-send code path's behavior.
                let local_name = if !state.user_name.is_empty() {
                    state.user_name.clone()
                } else {
                    "You".to_string()
                };
                let now = chrono_now_str();
                state.chat_messages.push(ChatMessage {
                    sender_name: local_name,
                    sender_key: state.profile_public_key.clone(),
                    content: pending.content.clone(),
                    timestamp: now,
                    timestamp_ms: pending.timestamp_ms,
                    channel: format!("dm:{}", pending.partner_key),
                    reply_to: None,
                    ..Default::default()
                });
                while state.chat_messages.len() > 200 {
                    state.chat_messages.remove(0);
                }
                state.chat_input.clear();
            }
        }
    }

    if close_modal {
        state.dm_unencrypted_confirm = None;
    }
}

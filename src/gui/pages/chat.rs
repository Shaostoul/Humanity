//! 3-panel chat page matching the website layout.
//!
//! LEFT:   Collapsible DMs (red), Groups (green), Servers (blue), Connection settings
//! MIDDLE: Channel header, message feed, input bar
//! RIGHT:  Friends list, Server members list

use egui::{Align, Color32, Frame, Layout, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::{ChatMessage, ChatUser, GuiState};
use crate::gui::theme::Theme;
use crate::gui::widgets;

// ── Extracted clusters (file-size ratchet) ──────────────────────────────────
// What stays in this file is the PAGE: the three-panel frame, the centre
// message feed, and the small helpers those share. Each cluster below lives in
// its own file under `pages/chat/` and takes `use super::*`, so it sees this
// file's imports and private helpers without any of them having to be widened.
// The names are imported back here so every call site reads exactly as it did
// before the move, and the `pub(crate)` half keeps
// `crate::gui::pages::chat::NAME` resolving for callers outside this page.

/// Group list, load, post, invite, leave, disband, and the peer-to-peer
/// signed-object traffic. See `chat/p2p_groups.rs`.
mod p2p_groups;
pub(crate) use p2p_groups::{
    broadcast_group_obj, drain_p2p_loaders, ensure_group_mesh, handle_p2p_group_obj,
    refresh_p2p_groups, spawn_group_load, spawn_groups_list_refresh,
};
use p2p_groups::{
    apply_group_load, disband_p2p_group, leave_p2p_group, mint_and_copy_p2p_invite,
    send_p2p_group_message,
};

/// The left rail: DMs, Groups, Commons, Servers and the expanded server's
/// channel list. See `chat/left_panel.rs`.
mod left_panel;
use left_panel::draw_left_panel;

/// The people surfaces: the Friends/Members right rail and the live strip.
/// See `chat/right_panel.rs`.
mod right_panel;
use right_panel::{draw_live_strip, draw_right_panel};

/// The overlays drawn on top of the panels: pins, search, the user profile,
/// add-server, edit-channel, create/join group, the Commons explainer, the
/// slash-command help, and the voice call. See `chat/modals.rs`.
mod modals;
pub(crate) use modals::{draw_call_bar, draw_incoming_call_modal, draw_user_modal};
use modals::{
    draw_add_server_modal, draw_commons_info_modal, draw_create_group_modal,
    draw_edit_channel_modal, draw_help_modal, draw_join_group_modal, draw_pins_modal,
    draw_search_modal,
};

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
    // File-attach picker modal (v0.708). On pick: validate, read, and upload
    // on a worker thread; the finished upload drains through the same
    // clipboard_upload receiver below and routes via send_composed_content.
    if let Some(mut picker) = state.chat_attach_picker.take() {
        use crate::gui::widgets::file_browser::{file_picker_modal, FilePickerResult};
        match file_picker_modal(ctx, theme, &mut picker, "Attach a file") {
            FilePickerResult::Open => {
                state.chat_attach_picker = Some(picker);
            }
            FilePickerResult::Cancelled => {}
            FilePickerResult::Picked(path) => {
                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
                    .to_string();
                match std::fs::read(&path) {
                    Ok(bytes) if (bytes.len() as u64) <= ATTACH_MAX_BYTES => {
                        let share = crate::gui::widgets::file_browser::name_matches_ext(
                            &filename,
                            SHARE_EXTS,
                        );
                        let mime = mime_for_filename(&filename).to_string();
                        let server = state.server_url.clone();
                        let pk = state.profile_public_key.clone();
                        // DM attachments are encrypted client-side (2026-08-24):
                        // the file must be as private as the message.
                        let is_dm = state.chat_active_channel.starts_with("dm:");
                        let (tx, rx) = std::sync::mpsc::channel();
                        std::thread::spawn(move || {
                            let result = prepare_attachment_send(
                                &server, &pk, &filename, &mime, bytes, share, is_dm,
                            )
                            .map_err(|e| e.to_string());
                            let _ = tx.send(result);
                        });
                        state.clipboard_upload =
                            Some((state.chat_active_channel.clone(), rx));
                    }
                    Ok(bytes) => {
                        log::warn!(
                            "Attach rejected: {} is {} bytes (max {})",
                            filename,
                            bytes.len(),
                            ATTACH_MAX_BYTES
                        );
                    }
                    Err(e) => {
                        log::warn!("Attach read failed for {filename}: {e}");
                    }
                }
            }
        }
    }

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
            // Pasting an image into a DM encrypts it too (2026-08-24).
            let is_dm = channel.starts_with("dm:");
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let result = prepare_attachment_send(
                    &server, &pk, "pasted-image.png", "image/png", png_bytes, false, is_dm,
                )
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
                state.clipboard_upload = None;
                // Route through the single content authority (v0.708): the
                // old code sent a raw chat message with the captured channel,
                // which BYPASSED DM encryption and the scratchpad's local-only
                // promise -- the same privacy-leak class the web client fixed
                // in v0.698.2. Sends to the CURRENT view, like the web.
                let sent = send_composed_content(state, &url);
                log::info!("Upload finished; routed send (sent={sent}): {url}");
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

    // ── LIVE STRIP (v0.1150, operator direction: "expandable tab at the top
    // of the chat page" for streaming). Declared BEFORE the side panels so
    // egui gives it the full window width above all three columns.
    draw_live_strip(ctx, theme, state);

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

    // 1:1 voice call surfaces (v0.703): the incoming-call Accept/Decline
    // modal + the in-call bar with Hang up.
    draw_incoming_call_modal(ctx, theme, state);
    draw_call_bar(ctx, theme, state);

    // (v0.847.x: the Create Channel modal was removed — it was never opened
    // since v0.187; channel creation lives in Server Settings > Channels.)

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

// ─────────────────────────────── CENTER PANEL ─────────────────────────────

fn draw_center_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // ── Channel header ──
    // Lock buttons moved OUT of the header in v0.189.x — operator wanted
    // them tucked into the actual panel CORNERS, not next to the channel
    // title where they could be mistaken for a UI label. They now paint
    // as floating Areas anchored to the side-panel boundaries (see the
    // overlays at the bottom of `chat::draw`).
    Frame::NONE
        .fill(theme.bg_sidebar_dark())
        .inner_margin(egui::Margin::symmetric(16, 10))
        .stroke(Stroke::new(1.0, theme.border()))
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
                    // DM header: back button + partner name. Plain "Back":
                    // the U+2190 arrow escape used here tofued (v0.723).
                    if widgets::Button::ghost("Back").tooltip("Return to #general").show(ui, theme) {
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
                            .color(theme.dm_accent())
                            .strong(),
                    );
                } else if let Some(gid) = ac.strip_prefix("p2pgroup:") {
                    // P2P group header: back button + name + a Copy-invite
                    // action. Leave / Disband live in the left-rail cog/
                    // right-click menu (kept out of this row so it can't
                    // overflow + clip on a narrow panel — that's why the
                    // operator couldn't reach them before).
                    if widgets::Button::ghost("Back").tooltip("Return to #general").show(ui, theme) {
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
                            .color(theme.group_accent())
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
                } else if let Some(room) = commons_room_of(&state.chat_active_channel) {
                    // Commons room header: bare room name + which of my
                    // servers bridge it, so the merged view names its
                    // sources instead of pretending to be one server.
                    let room = room.to_string();
                    ui.label(
                        RichText::new(format!("# {}", room))
                            .size(theme.font_size_heading)
                            .color(theme.text_primary())
                            .strong(),
                    );
                    let mut carriers: Vec<String> = Vec::new();
                    if state.chat_channels.iter().any(|c| c.id == room && c.federated)
                        && state.ws_client.as_ref().map_or(false, |c| c.is_connected())
                    {
                        carriers.push(server_display_name(&state.server_url));
                    }
                    for conn in state.connections.iter().filter(|c| c.identified) {
                        if conn.channels.iter().any(|c| c.id == room && c.federated) {
                            carriers.push(server_display_name(&conn.url));
                        }
                    }
                    ui.label(
                        RichText::new(format!("  |  Commons: {}", carriers.join(" + ")))
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
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

                // Commons merge (federation-ux.md): a Commons view merges the
                // conversation from every CARRIER of the bridged room -- and
                // ONLY carriers. A same-named local channel on a non-carrier
                // server is a different room and never blends in (the private
                // #general vs the bridged #general).
                // Dedup: a line native to one of my servers (origin_server
                // empty) beats its federated echoes; federated copies from
                // several carriers collapse on (origin, timestamp, content).
                let filtered: Vec<&ChatMessage> =
                    if let Some(room) = commons_room_of(&active_channel) {
                        let active_carries = state
                            .chat_channels
                            .iter()
                            .any(|c| c.id == room && c.federated);
                        let mut merged: Vec<&ChatMessage> = Vec::new();
                        if active_carries {
                            merged.extend(
                                state.chat_messages.iter().filter(|m| m.channel == room),
                            );
                        }
                        for conn in state.connections.iter() {
                            if conn.channels.iter().any(|c| c.id == room && c.federated) {
                                merged.extend(
                                    conn.messages.iter().filter(|m| m.channel == room),
                                );
                            }
                        }
                        let natives: std::collections::HashSet<(u64, &str)> = merged
                            .iter()
                            .filter(|m| m.origin_server.is_empty())
                            .map(|m| (m.timestamp_ms, m.content.as_str()))
                            .collect();
                        let mut seen_fed: std::collections::HashSet<(&str, u64, &str)> =
                            std::collections::HashSet::new();
                        let mut seen_native: std::collections::HashSet<(&str, u64, &str)> =
                            std::collections::HashSet::new();
                        merged.retain(|m| {
                            if m.origin_server.is_empty() {
                                // Two carriers can both hold the same native
                                // line (the snapshot repro proved the double
                                // render): collapse on sender + time + text.
                                return seen_native.insert((
                                    m.sender_key.as_str(),
                                    m.timestamp_ms,
                                    m.content.as_str(),
                                ));
                            }
                            if natives.contains(&(m.timestamp_ms, m.content.as_str())) {
                                return false; // the native original is present
                            }
                            seen_fed.insert((
                                m.origin_server.as_str(),
                                m.timestamp_ms,
                                m.content.as_str(),
                            ))
                        });
                        merged.sort_by_key(|m| m.timestamp_ms);
                        merged
                    } else {
                        state
                            .chat_messages
                            .iter()
                            .filter(|m| m.channel == active_channel)
                            .collect()
                    };

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
                            // A Commons view greets by the ROOM name, never
                            // the internal "commons:" id (field test 5).
                            let display_room =
                                commons_room_of(&active_channel).unwrap_or(&active_channel);
                            ui.label(
                                RichText::new(format!("Welcome to #{}", display_room))
                                    .size(theme.font_size_title)
                                    .color(theme.text_primary()),
                            );
                            ui.add_space(8.0);
                            let hint = if commons_room_of(&active_channel).is_some() {
                                "No messages here yet. History from your servers loads in \
                                 the background; new posts appear as they arrive."
                            } else {
                                "No messages yet. Say something!"
                            };
                            ui.label(
                                RichText::new(hint)
                                    .size(theme.font_size_body)
                                    .color(theme.text_muted()),
                            );
                        }
                    });
                }

                // Track alternating user colors. The even/odd row stripes are the
                // theme's dedicated table-striping tokens (bg_primary = base panel,
                // row_stripe = the subtle odd-row stripe egui also uses for lists),
                // so the message list restyles with the rest of the app.
                let mut last_sender = String::new();
                let mut sender_parity = false; // toggles each time sender changes
                let bg_even = theme.bg_primary();
                let bg_odd = theme.row_stripe();
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
                    // Encrypted DM attachment (2026-08-24): a FILE_MARKER carries
                    // an opaque ciphertext reference, not text or an image URL.
                    // Show a clean label instead of the raw base64 (full native
                    // inline decrypt is a tracked follow-up; the web client
                    // renders it inline today).
                    let enc_att = crate::net::dm_pq::parse_file_marker(&msg.content);
                    // Extract image URLs from the message so we can render them
                    // as inline thumbnails instead of raw /uploads/... text.
                    let image_urls = if enc_att.is_some() {
                        Vec::new()
                    } else {
                        crate::gui::widgets::image_cache::extract_image_urls(&msg.content)
                    };
                    let display_text = if let Some(ref att) = enc_att {
                        let kb = (att.size as f64 / 1024.0).max(1.0).round() as u64;
                        let kind = if att.mime.starts_with("image/") { "photo" } else { "file" };
                        format!("Encrypted {kind}: {} ({} KB). Open in the web app to view.", att.name, kb)
                    } else if image_urls.is_empty() {
                        msg.content.clone()
                    } else {
                        crate::gui::widgets::image_cache::strip_image_urls(&msg.content)
                    };
                    // Inline markdown + link detection (v0.702, parity with web:
                    // the help modal has advertised markdown all along). Markers
                    // are stripped HERE so the mention detection below and the
                    // row's char-indexed ranges all see the same display text.
                    // Fenced ```code``` blocks (v0.1208, parity with web's
                    // formatBody step 1): pull them OUT of the inline flow before
                    // bulletize/parse can mangle their contents (backticks, '*',
                    // etc.), and render each as a monospace panel with a Copy
                    // button below the message text. Mirrors how inline images
                    // are stripped here and drawn as separate widgets. The
                    // de-fenced text flows on through bulletize + parse as normal.
                    let (display_text, code_blocks) = extract_code_blocks(&display_text);
                    // Line-leading "- "/"* " -> a real bullet, parity with the
                    // web client's list rendering. Width-preserving and done
                    // BEFORE parse so the inline spans below stay char-aligned.
                    let display_text = bulletize_list_lines(&display_text);
                    let (display_text, format_spans) =
                        crate::gui::widgets::msg_format::parse(&display_text);
                    let link_targets: Vec<String> = format_spans
                        .iter()
                        .filter_map(|sp| match &sp.kind {
                            crate::gui::widgets::msg_format::SpanKind::Link(url) => {
                                Some(url.clone())
                            }
                            _ => None,
                        })
                        .collect();

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
                                        // Stable id (v0.972): the edit row
                                        // lives inside the scrolling message
                                        // list, where new messages shift
                                        // layout constantly - without a
                                        // fixed id an incoming message
                                        // dropped focus mid-edit.
                                        .id(egui::Id::new("chat_edit_input"))
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
                            &format_spans,
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
                        // Click on a link → OS default browser (same pattern
                        // as the Browser page's bookmarks). v0.702.
                        if let Some(idx) = row_resp.clicked_link {
                            if let Some(url) = link_targets.get(idx) {
                                ui.ctx().open_url(egui::OpenUrl::new_tab(url));
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

                    // Fenced code blocks (v0.1208): render each extracted ```
                    // block as an indented monospace panel with an optional
                    // language label + Copy button, directly under the message
                    // text. Skipped while editing (the raw ``` is in the editor).
                    if !is_editing && !code_blocks.is_empty() {
                        draw_code_blocks(ui, theme, &code_blocks);
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
                        // Palette lives in data/reactions.json (one source for
                        // native, relay allowlist, and web) via crate::reactions.
                        let top_reactions: &'static [String] = crate::reactions::top();
                        let all_reactions: &'static [String] = crate::reactions::all();
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
                            + top_reactions.len() as f32 * 28.0 // quick-row reactions
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
                                                for emoji in top_reactions {
                                                    let clean: String = emoji.chars().filter(|c| *c != '\u{FE0F}').collect(); // glyph-exempt: stripping the selector, not rendering it
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
                                                            for emoji in all_reactions {
                                                                let clean: String = emoji.chars().filter(|c| *c != '\u{FE0F}').collect(); // glyph-exempt: stripping the selector, not rendering it
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
            .fill(theme.bg_sidebar_dark())
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
                    // The message text field is a raised input surface -> the
                    // tertiary/extreme-bg token (what egui uses for text edits).
                    .fill(theme.bg_tertiary())
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
                    } else if let Some(room) = commons_room_of(&state.chat_active_channel) {
                        // Name the outgoing server EXPLICITLY: in the first
                        // field test a Commons send silently routed through a
                        // background carrier while the operator sat on an
                        // offline private server -- the composer must say
                        // where the message will actually go.
                        let active_carries = state
                            .chat_channels
                            .iter()
                            .any(|c| c.id == room && c.federated)
                            && state.ws_client.as_ref().map_or(false, |c| c.is_connected());
                        let via = if active_carries {
                            Some(server_display_name(&state.server_url))
                        } else {
                            state
                                .connections
                                .iter()
                                .find(|c| {
                                    c.identified
                                        && c.ws.is_some()
                                        && c.channels.iter().any(|ch| ch.id == room && ch.federated)
                                })
                                .map(|c| server_display_name(&c.url))
                        };
                        if commons_room_read_only(state, room) == Some(true) {
                            format!("#{} is read-only (announcements from the operators)", room)
                        } else {
                            match via {
                                Some(v) => format!("Message #{} (sends via {})", room, v),
                                None => format!("#{}: no connected server carries this room", room),
                            }
                        }
                    } else {
                        format!("Message #{}", state.chat_active_channel)
                    };
                    // Reserve room for the FOUR trailing widgets on this row —
                    // search, pins, help, and the Send button. 104px only fit the
                    // three icons, clipping Send off the right edge; ~210 fits all.
                    // Stable id (v0.972, operator: the "is typing" indicator
                    // appearing made the box lose focus): egui auto-ids
                    // derive from layout position, and the indicator row
                    // shifts the composer, so the focused id changed under
                    // the caret. A fixed id keeps focus across any layout
                    // shift (typing rows, reply banners, resizes).
                    let response = ui.add(
                        // Multiline (v0.1206.x composer expand) so a
                        // multi-paragraph post can be written AND proofread in
                        // place instead of a cramped single line (operator
                        // 2026-08-24: "never expands to show the whole post;
                        // editing is miserable"). desired_rows(1) keeps it
                        // one line tall until you actually write more, then it
                        // grows with the content. Enter still SENDS (the
                        // universal chat gesture + matches the web client);
                        // return_key = Shift+Enter makes Shift+Enter the
                        // newline. NOTE: with a non-plain return_key, egui does
                        // NOT surrender focus on plain Enter for a multiline box
                        // (see egui text_edit builder.rs ~L990) -- so the send
                        // trigger below uses has_focus()+Enter, NOT lost_focus().
                        egui::TextEdit::multiline(&mut state.chat_input)
                            .id(egui::Id::new("chat_composer_input"))
                            .desired_width((ui.available_width() - 210.0).max(120.0))
                            .desired_rows(1)
                            .return_key(Some(egui::KeyboardShortcut::new(
                                egui::Modifiers::SHIFT,
                                egui::Key::Enter,
                            )))
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
                    //
                    // The composer is now a multiline TextEdit with
                    // return_key = Shift+Enter, so plain Enter is NOT consumed
                    // by the box (it neither inserts a newline nor surrenders
                    // focus). We therefore detect the send on has_focus()+Enter
                    // (shift excluded, since Shift+Enter is the newline) rather
                    // than the old singleline lost_focus() path.
                    let enter_pressed = !mention_took_enter
                        && response.has_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);

                    // Search button — opens the message search modal.
                    if widgets::Button::ghost("🔍").tooltip("Search messages in this channel").show(ui, theme) {
                        state.chat_search_open = true;
                    }

                    // Pins button — shows pin count + opens the pins modal.
                    let pin_count = state.chat_pins.get(&state.chat_active_channel).map(|p| p.len()).unwrap_or(0);
                    let pin_label = if pin_count > 0 { format!("📌 {}", pin_count) } else { "📌".to_string() };
                    if widgets::Button::ghost(&pin_label).tooltip("Pinned messages in this channel").show(ui, theme) {
                        state.chat_pins_open = true;
                    }

                    // Help button (?) - opens slash commands reference
                    if widgets::Button::ghost("?").tooltip("Slash-command reference and formatting tips").show(ui, theme) {
                        state.show_help_modal = !state.show_help_modal;
                    }

                    // In-app file attach (v0.708, all-in-one direction: our
                    // OWN browser widget, not an OS dialog).
                    if widgets::Button::secondary("Attach")
                        .tooltip("Attach a file from your computer (images, documents, \
                                  3D models - up to 6 MB). 3D files are also published \
                                  to the server's Shared Files library.")
                        .show(ui, theme)
                    {
                        state.chat_attach_picker = Some(
                            crate::gui::widgets::file_browser::FilePickerState::new(
                                ATTACH_EXTS,
                                ATTACH_MAX_BYTES,
                            ),
                        );
                    }
                    let send_clicked = widgets::Button::primary("Send")
                        .tooltip("Send the message (or press Enter)")
                        .show(ui, theme);

                    if (enter_pressed || send_clicked) && !state.chat_input.trim().is_empty() {
                        let content = state.chat_input.trim().to_string();
                        // ALL content routing (P2P group / scratchpad / E2EE
                        // DM / group / channel + the local echo) lives in
                        // send_composed_content -- the single authority
                        // shared with the clipboard-paste and file-attach
                        // flows so the three paths can never drift. (The web
                        // client had exactly that drift as the DM-attachment
                        // privacy leak, fixed v0.698.2; native's clipboard
                        // flow had the same class of bug until v0.708.)
                        if send_composed_content(state, &content) {
                            state.chat_input.clear();
                            response.request_focus();
                        }
                    }

                    if enter_pressed {
                        response.request_focus();
                    }
                });
                });
            });
    });
}

/// Full-width press-and-HOLD confirmation button for a destructive native
/// action -- the button-shaped twin of `widgets::hold_to_confirm`'s icon-sized
/// pinwheel. A danger bar fills over `hold_seconds` while the pointer is held;
/// releasing resets to zero. Returns true exactly once, on the frame the hold
/// completes, so a single click can never fire the action. `idle` / `held` are
/// the labels shown before and during the hold. Uses the same `ctx.data_mut`
/// progress technique as the shipped `hold_to_confirm` widget.
///
/// `pub(crate)` so the Settings "Widgets" effects test bench can demo the exact
/// same button the destructive chat actions use.
pub(crate) fn hold_to_confirm_button(
    ui: &mut egui::Ui,
    theme: &Theme,
    id: egui::Id,
    idle: &str,
    held: &str,
    hold_seconds: f32,
    tooltip: &str,
) -> bool {
    let w = ui.available_width().max(180.0);
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(w, 30.0), egui::Sense::click_and_drag());
    let mut progress: f32 = ui.ctx().data_mut(|d| d.get_temp(id).unwrap_or(0.0));
    let holding = resp.is_pointer_button_down_on();
    if holding {
        let dt = ui.input(|i| i.stable_dt).min(0.1);
        progress += dt / hold_seconds.max(0.1);
    } else {
        progress = 0.0;
    }
    let fired = progress >= 1.0;
    if fired {
        progress = 0.0;
    }
    ui.ctx().data_mut(|d| d.insert_temp(id, progress));

    // A clock-style ring on the left fills as you hold, and the arc cycles RGB
    // FAST (faster than the 3s nav channeling) so the wheel is as visible as
    // possible - the operator found the old full-width fill bar "a bit much" and
    // the ring more pleasant. The whole row is still the hold target.
    if resp.hovered() || holding {
        ui.painter().rect_filled(rect, Rounding::same(4), theme.bg_card());
    }
    let ring_r = 10.0;
    let ring_c = egui::pos2(rect.left() + 6.0 + ring_r, rect.center().y);
    // Faint full-circle track.
    ui.painter()
        .circle_stroke(ring_c, ring_r, Stroke::new(3.0, theme.border()));
    // Progress arc from 12 o'clock, clockwise, in a fast-cycling RGB colour.
    if progress > 0.0 {
        let now = ui.input(|i| i.time);
        let col = crate::gui::widgets::row::rgb_from_time(now * 15.0); // ~0.8s full cycle
        let n = 48usize;
        let filled = ((n as f32 * progress).ceil() as usize).clamp(1, n);
        let mut pts = Vec::with_capacity(filled + 1);
        for k in 0..=filled {
            let a = -std::f32::consts::FRAC_PI_2
                + std::f32::consts::TAU * (k as f32 / n as f32);
            pts.push(egui::pos2(
                ring_c.x + ring_r * a.cos(),
                ring_c.y + ring_r * a.sin(),
            ));
        }
        ui.painter()
            .add(egui::Shape::line(pts, Stroke::new(3.0, col)));
    }
    // Label to the right of the ring.
    let label = if holding { held } else { idle };
    let txt = if holding { theme.danger() } else { theme.text_primary() };
    ui.painter().text(
        egui::pos2(ring_c.x + ring_r + 10.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(theme.font_size_small),
        txt,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if holding {
        ui.ctx().request_repaint();
    }
    resp.on_hover_text(tooltip);
    fired
}

/// One fenced code block pulled out of a chat message by `extract_code_blocks`.
struct CodeBlock {
    /// Optional language token from the opening fence (```rust -> "rust"). May
    /// be empty. Shown as a small label; not used for syntax highlighting yet.
    lang: String,
    /// The code between the fences, verbatim (newlines preserved).
    code: String,
}

/// Pull fenced ```code``` blocks out of a message body, mirroring the web
/// client's `formatBody` step 1 (`/```(\w*)\n?([\s\S]*?)```/`). Returns the text
/// with the fenced regions removed and the ordered list of blocks to render as
/// separate monospace panels. Extracting BEFORE the inline parser is what keeps a
/// code block's own backticks / `*` / URLs from being mis-parsed as inline
/// markup. An UNCLOSED opening fence is left verbatim (nothing is silently eaten),
/// matching the "unclosed marker renders as text" rule in msg_format.
fn extract_code_blocks(s: &str) -> (String, Vec<CodeBlock>) {
    const FENCE: &str = "```";
    if !s.contains(FENCE) {
        return (s.to_string(), Vec::new());
    }
    let mut text = String::with_capacity(s.len());
    let mut blocks: Vec<CodeBlock> = Vec::new();
    let mut rest = s;
    loop {
        match rest.find(FENCE) {
            None => {
                text.push_str(rest);
                break;
            }
            Some(open) => {
                let after_open = &rest[open + FENCE.len()..];
                match after_open.find(FENCE) {
                    None => {
                        // No closing fence: leave the rest (incl. the ```) as text.
                        text.push_str(rest);
                        break;
                    }
                    Some(close_rel) => {
                        text.push_str(&rest[..open]);
                        let inner = &after_open[..close_rel];
                        let (lang, code) = split_fence_lang(inner);
                        blocks.push(CodeBlock { lang, code });
                        let tail = &after_open[close_rel + FENCE.len()..];
                        // Swallow one newline right after the closing fence so the
                        // following prose doesn't render with a leading blank line
                        // where the block used to sit.
                        rest = tail
                            .strip_prefix("\r\n")
                            .or_else(|| tail.strip_prefix('\n'))
                            .unwrap_or(tail);
                    }
                }
            }
        }
    }
    // Trim the blank lines the extraction can leave where a block used to be, so
    // the surrounding prose doesn't render with stray empty rows.
    let text = text.trim_matches('\n').to_string();
    (text, blocks)
}

/// Split the inside of a fence into an optional language token and the code,
/// matching web's `formatBody` regex `/```(\w*)\n?([\s\S]*?)```/` exactly so the
/// two clients read the same dialect: a leading run of word chars (`[A-Za-z0-9_]`)
/// is the language, then ONE optional newline is consumed, and the rest (trailing
/// newlines trimmed) is the code.
fn split_fence_lang(inner: &str) -> (String, String) {
    let lang_end = inner
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_'))
        .map(|(i, _)| i)
        .unwrap_or(inner.len());
    let lang = &inner[..lang_end];
    let mut code = &inner[lang_end..];
    // Consume a single optional newline right after the language token (CRLF too).
    if let Some(stripped) = code.strip_prefix("\r\n") {
        code = stripped;
    } else if let Some(stripped) = code.strip_prefix('\n') {
        code = stripped;
    }
    (lang.to_string(), code.trim_end_matches(['\n', '\r']).to_string())
}

/// Draw extracted fenced code blocks as indented monospace panels, each with an
/// optional language label and a Copy button (parity with web's
/// `code-block-wrapper`). Rendered directly under the message text.
fn draw_code_blocks(ui: &mut egui::Ui, theme: &Theme, blocks: &[CodeBlock]) {
    let indent = theme.avatar_size + theme.avatar_gap;
    let panel_w = (ui.available_width() - indent - 8.0).max(80.0);
    for cb in blocks {
        ui.horizontal(|ui| {
            ui.add_space(indent);
            ui.vertical(|ui| {
                // Bound the width so long code lines wrap inside the column
                // instead of stretching the horizontal layout off the edge.
                ui.set_max_width(panel_w);
                Frame::none()
                    .fill(theme.bg_card())
                    .stroke(Stroke::new(theme.border_width, theme.border()))
                    .rounding(Rounding::same(4))
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if !cb.lang.is_empty() {
                                ui.label(
                                    RichText::new(&cb.lang)
                                        .monospace()
                                        .size(theme.small_size)
                                        .color(theme.text_muted()),
                                );
                            }
                            if widgets::Button::ghost("Copy").show(ui, theme) {
                                ui.ctx().copy_text(cb.code.clone());
                            }
                        });
                        ui.label(
                            RichText::new(&cb.code)
                                .monospace()
                                .color(theme.text_primary()),
                        );
                    });
            });
        });
        ui.add_space(2.0);
    }
}

/// Render line-leading list markers as a real bullet on native, matching the
/// web client, whose `formatBody` turns `- x` / `* x` into a `<ul>` list.
///
/// This is a WIDTH-PRESERVING text substitution done BEFORE
/// `msg_format::parse`, which is the whole reason it's safe: the 2-char marker
/// `"- "` / `"* "` becomes the 2-char `"\u{2022} "` (bullet + space), so the
/// char-indexed inline spans (bold/italic/mention/link) computed by `parse`
/// stay perfectly aligned. `\u{2022}` (•) is in the General Punctuation block,
/// which the app font renders reliably.
///
/// Requires the trailing space (same rule as web's `/^[-*] /`), so an inline
/// `*italic*` and a bare `-` in prose are left alone. Leading indentation is
/// preserved so nested-looking lists keep their indent. Block-level quotes and
/// headings remain a follow-up (native `msg_format` is inline-only).
fn bulletize_list_lines(s: &str) -> String {
    // Fast path: the overwhelmingly common message has no line-leading marker,
    // so avoid rebuilding the string. `.any` over the (usually one) line is cheap.
    let has_marker = s.split('\n').any(|l| {
        let t = l.trim_start();
        t.starts_with("- ") || t.starts_with("* ")
    });
    if !has_marker {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    for (i, line) in s.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let indent_len = line.len() - line.trim_start().len();
        let (indent, rest) = line.split_at(indent_len);
        if let Some(after) = rest.strip_prefix("- ").or_else(|| rest.strip_prefix("* ")) {
            out.push_str(indent);
            out.push('\u{2022}'); // • bullet, General Punctuation (font-safe)
            out.push(' ');
            out.push_str(after);
        } else {
            out.push_str(line);
        }
    }
    out
}

/// Human-readable label for a chat channel id (v0.772): server channels show
/// `#name`, DMs and group channels get a friendly prefix instead of the raw
/// `dm:<key>` / `p2pgroup:<id>` string.
pub(crate) fn channel_display_label(channel: &str) -> String {
    if let Some(rest) = channel.strip_prefix("dm:") {
        // chars-safe truncation (v0.779): byte-slicing `&rest[..8]` panicked on
        // a non-char-boundary if a relay ever supplied a non-ASCII key, and the
        // HUD calls this every frame.
        let short: String = rest.chars().take(8).collect();
        format!("DM {short}")
    } else if channel.starts_with("p2pgroup:") {
        "Group".to_string()
    } else {
        format!("#{channel}")
    }
}

/// Ordering for the in-world DM list: most recent first (increment 1c).
/// `ChatDm.timestamp` is a DISPLAY string (see `format_timestamp`), not epoch
/// millis -- but every entry alive at one moment was formatted with the SAME
/// app-wide format, and each of those formats orders lexicographically within
/// itself ("2026-05-29 17:42 UTC" is ISO-ordered; "17:42 UTC" orders within a
/// day). A descending lexicographic sort of the display strings is therefore
/// the correct recency order in practice, WITHOUT duplicating the DM store or
/// threading epoch millis through ChatDm (the "reuse the Chat page's state,
/// don't build a second store" rule). Entries with NO timestamp (a
/// conversation opened but never messaged) sink to the end. Stable: ties keep
/// their incoming order (alphabetical -- the Chat page sorts the store).
/// Returns indices into `timestamps`.
pub(crate) fn dm_recency_order(timestamps: &[&str]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..timestamps.len()).collect();
    idx.sort_by(|&a, &b| {
        let (ta, tb) = (timestamps[a], timestamps[b]);
        match (ta.is_empty(), tb.is_empty()) {
            (true, true) => std::cmp::Ordering::Equal,
            // Timestamp-less conversations sink below every dated one.
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            // Descending string compare = most recent first (see doc above).
            (false, false) => tb.cmp(ta),
        }
    });
    idx
}

/// Header title for the in-world chat panel, resolved per view mode: the
/// active channel label in Channels mode, the DM partner's NAME (not the raw
/// key prefix `channel_display_label` falls back to) in DMs mode, the group's
/// name in Groups mode, and a static caption for the list/options views.
fn ingame_panel_title(state: &GuiState) -> String {
    let ac = &state.chat_active_channel;
    match state.ingame_chat_mode {
        crate::gui::IngameChatMode::Channels => channel_display_label(ac),
        crate::gui::IngameChatMode::Dms => {
            if let Some(pk) = ac.strip_prefix("dm:") {
                let name = state
                    .chat_dms
                    .iter()
                    .find(|d| d.user_key == pk)
                    .map(|d| d.user_name.clone())
                    // Unknown partner (store not synced yet): short key prefix,
                    // chars-safe like channel_display_label.
                    .unwrap_or_else(|| pk.chars().take(8).collect());
                format!("DM {name}")
            } else {
                "Direct Messages".to_string()
            }
        }
        crate::gui::IngameChatMode::Groups => {
            if let Some(gid) = ac.strip_prefix("p2pgroup:") {
                state
                    .p2p_groups
                    .iter()
                    .find(|g| g.group_id == gid)
                    .map(|g| g.name.clone())
                    .unwrap_or_else(|| "Group".to_string())
            } else {
                "Groups".to_string()
            }
        }
        crate::gui::IngameChatMode::Options => "Chat Options".to_string(),
    }
}

/// In-world chat panel (v0.772, unified-chat increment 1b; view modes added in
/// increment 1c): opened with Enter while playing the 3D world (which sets
/// `chat_input_active`, freeing the cursor + disabling look/move in lib.rs).
/// A compact bottom-left panel over the world with a mode tab row --
/// [Chat] [DMs] [Groups] [...] -- so channels, DM conversations, group chats,
/// and a tiny options slice are all reachable without leaving the world. Every
/// conversation store and send path is SHARED with the Chat page
/// (`send_composed_content` routes channel / E2EE-DM / group / P2P-group), so
/// the two surfaces can never drift. Esc or Close returns to gameplay; the
/// mode is session-persistent so the panel reopens where you left off.
pub(crate) fn draw_ingame_chat(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // (Esc-close is owned by the winit modal guard in lib.rs, which fires
    // before egui sees the key -- one close path, shared with the creature
    // editor, instead of two drifting copies. v0.779)
    let active = state.chat_active_channel.clone();
    let connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    let mut close = false;
    // Deferred actions, applied after the Area closure so the render pass
    // never mutates the lists it is iterating (same pattern as the Chat page).
    let mut switch_to: Option<String> = None; // public channel id
    let mut open_dm: Option<String> = None; // DM partner key
    let mut open_p2p_group: Option<String> = None; // P2pGroupInfo.group_id
    let mut open_full_chat = false;

    // Which mode has an active conversation the message list + input serve.
    let dm_open = active.starts_with("dm:");
    let group_open = active.starts_with("p2pgroup:") || active.starts_with("group:");

    let area = egui::Area::new(egui::Id::new("ingame_chat_panel"))
        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(12.0, -12.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.set_width(470.0);
                    // Header: mode-resolved title + connection state + close.
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(ingame_panel_title(state))
                                .strong()
                                .color(theme.accent()),
                        );
                        ui.label(
                            RichText::new(if connected { "connected" } else { "offline" })
                                .size(theme.font_size_small)
                                .color(if connected { theme.success() } else { theme.warning() }),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Close (Esc)").clicked() {
                                close = true;
                            }
                        });
                    });

                    // Mode tab row (increment 1c): [Chat] [DMs] [Groups] [...].
                    // Same visual family as the channel switcher below
                    // (selectable labels), with a small unread dot painted on a
                    // tab whose section has activity you haven't seen.
                    ui.horizontal(|ui| {
                        for m in crate::gui::IngameChatMode::ALL {
                            let selected = state.ingame_chat_mode == m;
                            let resp = ui.selectable_label(selected, m.label());
                            let has_unread = match m {
                                crate::gui::IngameChatMode::Channels => {
                                    state.chat_channels.iter().any(|c| c.unread)
                                }
                                crate::gui::IngameChatMode::Dms => {
                                    state.chat_dms.iter().any(|d| d.unread)
                                }
                                // P2P groups carry no unread flag yet (the
                                // projection has no read tracking), so no dot
                                // for Groups since the legacy retirement.
                                crate::gui::IngameChatMode::Groups => false,
                                crate::gui::IngameChatMode::Options => false,
                            };
                            if has_unread {
                                // Top-right corner dot, same red family as the
                                // sidebar unread dots but via the theme token.
                                ui.painter().circle_filled(
                                    resp.rect.right_top() + egui::vec2(-2.0, 4.0),
                                    2.5,
                                    theme.danger(),
                                );
                            }
                            if resp.clicked() && !selected {
                                state.ingame_chat_mode = m;
                                // Entering Channels with a private conversation
                                // still active would render DM/group text under
                                // the public tab -- snap back to a public
                                // channel (first known, else #general) exactly
                                // like the v0.779 open-guard did.
                                if m == crate::gui::IngameChatMode::Channels
                                    && (active.starts_with("dm:")
                                        || active.starts_with("p2pgroup:")
                                        || active.starts_with("group:"))
                                {
                                    let fallback = state
                                        .chat_channels
                                        .first()
                                        .map(|c| c.id.clone())
                                        .unwrap_or_else(|| "general".to_string());
                                    switch_to = Some(fallback);
                                }
                            }
                        }
                    });
                    ui.separator();

                    // Computed AFTER the tab row so a tab click renders its
                    // mode's full layout (list + messages + input) in the same
                    // frame instead of one frame of mixed content. Channels
                    // requires a PUBLIC active channel: on the click frame of a
                    // Chat-tab snap-back the private channel is still active
                    // (switch_to applies after the Area), and rendering its
                    // rows under the public tab -- even for one frame -- would
                    // leak private text.
                    let show_messages = match state.ingame_chat_mode {
                        crate::gui::IngameChatMode::Channels => !dm_open && !group_open,
                        crate::gui::IngameChatMode::Dms => dm_open,
                        crate::gui::IngameChatMode::Groups => group_open,
                        crate::gui::IngameChatMode::Options => false,
                    };
                    let show_input = show_messages;

                    match state.ingame_chat_mode {
                        crate::gui::IngameChatMode::Channels => {
                            // Channel switcher: the server's channels, as-is (no
                            // invented fallback rows -- a fabricated #announcements
                            // button on a server without that channel would switch
                            // you into a channel that does not exist; the Chat page
                            // sidebar renders the same list unpadded. Empty list =
                            // no switcher row. v0.779).
                            let chans: Vec<(String, String, bool)> = state
                                .chat_channels
                                .iter()
                                .map(|c| (c.id.clone(), c.name.clone(), c.unread))
                                .collect();
                            ui.horizontal_wrapped(|ui| {
                                for (id, name, unread) in &chans {
                                    let is_active = active == *id;
                                    let resp = ui.selectable_label(is_active, format!("#{name}"));
                                    if *unread && !is_active {
                                        ui.painter().circle_filled(
                                            resp.rect.right_top() + egui::vec2(-2.0, 4.0),
                                            2.5,
                                            theme.danger(),
                                        );
                                    }
                                    if resp.clicked() && !is_active {
                                        switch_to = Some(id.clone());
                                    }
                                }
                            });
                        }
                        crate::gui::IngameChatMode::Dms => {
                            // DM conversation switcher: most recent first (the
                            // Chat page sidebar is alphabetical; in-world you
                            // want "who talked to me last" at the front). Rows
                            // come straight from the Chat page's store --
                            // clicking one runs the SAME open path (dm_open +
                            // history fetch), so read-state stays in sync.
                            let dms: Vec<(String, String, bool, String)> = state
                                .chat_dms
                                .iter()
                                .map(|d| {
                                    (
                                        d.user_key.clone(),
                                        d.user_name.clone(),
                                        d.unread,
                                        d.timestamp.clone(),
                                    )
                                })
                                .collect();
                            if dms.is_empty() {
                                ui.label(
                                    RichText::new(
                                        "No conversations yet. Start one from a member's profile on the Chat page.",
                                    )
                                    .color(theme.text_muted())
                                    .size(theme.font_size_small),
                                );
                            } else {
                                let ts_refs: Vec<&str> =
                                    dms.iter().map(|d| d.3.as_str()).collect();
                                let order = dm_recency_order(&ts_refs);
                                ui.horizontal_wrapped(|ui| {
                                    for i in order {
                                        let (key, name, unread, _) = &dms[i];
                                        let is_active = active == format!("dm:{key}");
                                        let resp =
                                            ui.selectable_label(is_active, format!("@ {name}"));
                                        if *unread && !is_active {
                                            ui.painter().circle_filled(
                                                resp.rect.right_top() + egui::vec2(-2.0, 4.0),
                                                2.5,
                                                theme.danger(),
                                            );
                                        }
                                        if resp.clicked() && !is_active {
                                            open_dm = Some(key.clone());
                                        }
                                    }
                                });
                            }
                            // Locked/absent identity guidance -- same wording
                            // family as the Chat page's connect panel, because
                            // DMs need the seed for the post-quantum key. The
                            // unlock button routes to the SAME passphrase/PIN
                            // modal (it draws over everything, world included).
                            let identity_locked = state.private_key_bytes.is_none()
                                && !state.encrypted_private_key.is_empty();
                            if identity_locked {
                                ui.add_space(theme.spacing_xs);
                                ui.label(
                                    RichText::new(
                                        "Identity locked. Your seed is encrypted; unlock it to send DMs (they need it for the post-quantum key).",
                                    )
                                    .color(theme.warning())
                                    .size(theme.font_size_small),
                                );
                                let (btn_label, target_mode) = match state.auto_unlock_mode {
                                    crate::auto_unlock::AutoUnlockMode::KeychainPin
                                        if !state.pin_encrypted_seed.is_empty() =>
                                    {
                                        ("Unlock with PIN", crate::gui::PassphraseMode::PinUnlock)
                                    }
                                    _ => ("Unlock with Passphrase", crate::gui::PassphraseMode::Unlock),
                                };
                                if ui.button(btn_label).clicked() {
                                    state.passphrase_needed = true;
                                    state.passphrase_mode = target_mode;
                                }
                            } else if state.private_key_bytes.is_none() {
                                ui.add_space(theme.spacing_xs);
                                ui.label(
                                    RichText::new(
                                        "No identity on this device yet. Set one up on the Chat page (Connect) to use DMs.",
                                    )
                                    .color(theme.text_muted())
                                    .size(theme.font_size_small),
                                );
                            }
                            if !dm_open && !dms.is_empty() {
                                ui.add_space(theme.spacing_xs);
                                ui.label(
                                    RichText::new("Select a conversation above.")
                                        .color(theme.text_muted())
                                        .size(theme.font_size_small),
                                );
                            }
                        }
                        crate::gui::IngameChatMode::Groups => {
                            // Group switcher: the E2EE P2P groups (the only
                            // kind since the 2026-08-23 legacy retirement).
                            let p2p: Vec<(String, String)> = state
                                .p2p_groups
                                .iter()
                                .map(|g| (g.group_id.clone(), g.name.clone()))
                                .collect();
                            if p2p.is_empty() {
                                ui.label(
                                    RichText::new(
                                        "No groups yet. Create or join one on the Chat page.",
                                    )
                                    .color(theme.text_muted())
                                    .size(theme.font_size_small),
                                );
                            } else {
                                ui.horizontal_wrapped(|ui| {
                                    for (gid, name) in &p2p {
                                        let is_active = active == format!("p2pgroup:{gid}");
                                        let resp = ui.selectable_label(is_active, name);
                                        if resp.clicked() && !is_active {
                                            open_p2p_group = Some(gid.clone());
                                        }
                                    }
                                });
                                if !group_open {
                                    ui.add_space(theme.spacing_xs);
                                    ui.label(
                                        RichText::new("Select a group above.")
                                            .color(theme.text_muted())
                                            .size(theme.font_size_small),
                                    );
                                }
                            }
                        }
                        crate::gui::IngameChatMode::Options => {
                            // Tiny settings slice -- the panel's own knobs only;
                            // everything else lives on the full Chat page.
                            if ui
                                .checkbox(
                                    &mut state.hud_chat_feed_visible,
                                    "Show chat feed in world (bottom-left)",
                                )
                                .on_hover_text(
                                    "The passive read-only message feed painted on the HUD while you play.",
                                )
                                .changed()
                            {
                                crate::config::AppConfig::from_gui_state(state).save();
                            }
                            ui.add_space(theme.spacing_xs);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Panel height")
                                        .size(theme.font_size_small)
                                        .color(theme.text_secondary()),
                                );
                                let resp = ui.add(
                                    egui::Slider::new(
                                        &mut state.ingame_chat_panel_height,
                                        100.0..=320.0,
                                    )
                                    .suffix(" px"),
                                );
                                // Persist on release, not every drag frame --
                                // config save hits the disk.
                                if resp.drag_stopped() {
                                    crate::config::AppConfig::from_gui_state(state).save();
                                }
                            });
                            ui.add_space(theme.spacing_sm);
                            if ui.button("Open full Chat page").clicked() {
                                open_full_chat = true;
                            }
                        }
                    }

                    if show_messages {
                        ui.separator();
                        // Message list for the active conversation (scrollable,
                        // newest at bottom). DM content is already decrypted at
                        // receive time (lib.rs), so rendering is uniform.
                        egui::ScrollArea::vertical()
                            .max_height(state.ingame_chat_panel_height)
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                let msgs: Vec<(&str, &str)> = state
                                    .chat_messages
                                    .iter()
                                    .filter(|m| m.channel == active)
                                    .map(|m| {
                                        (
                                            if m.sender_name.is_empty() { "?" } else { m.sender_name.as_str() },
                                            m.content.as_str(),
                                        )
                                    })
                                    .collect();
                                if msgs.is_empty() {
                                    ui.label(
                                        RichText::new("No messages yet.")
                                            .color(theme.text_muted())
                                            .size(theme.font_size_small),
                                    );
                                }
                                for (name, content) in msgs.iter().rev().take(40).rev() {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(
                                            RichText::new(format!("{name}:"))
                                                .strong()
                                                .color(theme.text_primary())
                                                .size(theme.font_size_small),
                                        );
                                        ui.label(
                                            RichText::new(*content)
                                                .color(theme.text_secondary())
                                                .size(theme.font_size_small),
                                        );
                                    });
                                }
                            });
                    }

                    if show_input {
                        // Input line: auto-focused on open, Enter or Send posts
                        // to the active conversation via the same path the Chat
                        // page uses (send_composed_content routes channel /
                        // E2EE-DM / group / P2P-group identically).
                        ui.horizontal(|ui| {
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut state.chat_input)
                                    // Stable id (v0.972): same focus-loss
                                    // protection as the main composer.
                                    .id(egui::Id::new("chat_overlay_input"))
                                    .desired_width(390.0)
                                    .hint_text("Message... (Enter to send, Esc to close)"),
                            );
                            if state.chat_input_focus_pending {
                                resp.request_focus();
                                state.chat_input_focus_pending = false;
                            }
                            let enter_send =
                                resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                            let button_send = ui.button("Send").clicked();
                            if enter_send || button_send {
                                let text = state.chat_input.trim().to_string();
                                if !text.is_empty() {
                                    // Respect the abort contract (v0.779): false means
                                    // the send did NOT go out (unencryptable DM stashed
                                    // for the confirm modal, failed P2P-group send) --
                                    // keep the typed text so it isn't destroyed, same
                                    // as the Chat page composer.
                                    if send_composed_content(state, &text) {
                                        state.chat_input.clear();
                                    }
                                }
                                // Keep the panel open + refocused so you can keep chatting.
                                resp.request_focus();
                            }
                        });
                    }
                });
        });

    // Click-away closes the panel but KEEPS the typed text (v0.773): the big
    // panel shouldn't linger over the world (it blocks the view in combat). A
    // click whose position falls OUTSIDE the panel rect returns to gameplay;
    // chat_input is left intact so reopening (Enter) resumes the message.
    // Suppressed while an overlay modal owns the screen (the passphrase
    // prompt): that window sits OUTSIDE the panel rect, and clicking its
    // buttons must not also close the panel.
    let clicked_outside = ctx.input(|i| i.pointer.any_click())
        && ctx
            .input(|i| i.pointer.interact_pos())
            .map_or(false, |p| !area.response.rect.contains(p))
        && !state.passphrase_needed;

    if let Some(id) = switch_to {
        state.chat_active_channel = id.clone();
        // Full switch, mirroring the Chat page (v0.779): CLEAR the shared
        // message vec so the re-fetched history doesn't append older messages
        // after newer live ones (nothing re-sorts on this path), and clear the
        // channel's unread dot like opening it on the Chat page does.
        state.chat_messages.clear();
        state.history_fetched = false;
        if let Some(c) = state.chat_channels.iter_mut().find(|c| c.id == id) {
            c.unread = false;
        }
    }
    if let Some(key) = open_dm {
        // SAME open path as the Chat page's DM row click (draw_dm_section):
        // switch the channel and load the conversation from the LOCAL
        // encrypted store (sealed-sender: the relay keeps no DM history).
        open_dm_conversation(state, &key);
    }
    if let Some(gid) = open_p2p_group {
        // SAME open path as the Chat page's P2P group row click: switch the
        // channel instantly, then load + decrypt on a background thread
        // (spawn_group_load) so the world render never freezes.
        state.chat_active_channel = format!("p2pgroup:{gid}");
        state.p2p_group_invite_status.clear();
        state.chat_reply_to = None;
        // Drop the previous group's rows immediately so we don't briefly
        // show stale history under the new header.
        state.chat_messages.retain(|m| !m.channel.starts_with("p2pgroup:"));
        spawn_group_load(state, &gid, true);
    }
    if open_full_chat {
        // Hand off to the full Chat page: close the panel (keeping the typed
        // text, same as click-away) and navigate. The per-frame cursor
        // reconciliation in lib.rs handles the page-mode cursor.
        state.active_page = crate::gui::GuiPage::Chat;
        state.chat_input_active = false;
        state.chat_input_focus_pending = false;
    }
    if close || clicked_outside {
        state.chat_input_active = false;
        state.chat_input_focus_pending = false;
    }

}

/// THE single content-routing authority for native chat (v0.708), mirroring
/// the web's `sendComposedContent` (v0.698.2): a typed message, a pasted
/// image URL, and an attached file URL all flow through HERE, so the
/// P2P-group / scratchpad / E2EE-DM (fail-closed) / group / channel routing
/// and the local echo can never drift between input paths. Returns true when
/// the content was sent (or locally echoed for the scratchpad); false when
/// the send was aborted (unencryptable DM stashed for the confirm modal, or
/// a failed P2P-group send) -- the composer keeps its input in that case.
fn send_composed_content(state: &mut GuiState, content: &str) -> bool {
    let channel = state.chat_active_channel.clone();
    // Single timestamp for both the WS send and the local echo so
    // reaction-targeting (which keys on sender + ts) matches.
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let mut send_aborted = false;

    // P2P groups send over HTTP (signed objects), not the WS relay.
    let is_p2p_group = channel.starts_with("p2pgroup:");
    if is_p2p_group && !send_p2p_group_message(state, &channel, content) {
        send_aborted = true;
    }

    // The scratchpad is LOCAL-ONLY, as its label promises (v0.702 fix).
    let is_scratchpad = channel == "scratchpad";

    // Commons routing (federation-ux.md): a Commons view ("commons:<room>")
    // sends into the underlying bridged room. If the ACTIVE server carries
    // it (a federated channel of that name), send there; otherwise route
    // through a background carrier connection -- never onto a relay that
    // does not carry the room (which would fork a same-named LOCAL,
    // unbridged channel there). Fail closed when NOTHING carries it.
    let commons_room: Option<String> = commons_room_of(&channel).map(|s| s.to_string());
    let wire_channel = commons_room.clone().unwrap_or_else(|| channel.clone());
    let active_carries_room = match commons_room.as_deref() {
        Some(room) => {
            state.ws_client.as_ref().map_or(false, |c| c.is_connected())
                && state.chat_channels.iter().any(|c| c.id == room && c.federated)
        }
        None => true, // plain channels always use the active socket
    };
    let carrier_idx: Option<usize> = match commons_room.as_deref() {
        Some(room) if !active_carries_room => state.connections.iter().position(|c| {
            c.identified
                && c.ws.is_some()
                && c.channels.iter().any(|ch| ch.id == room && ch.federated)
        }),
        _ => None,
    };
    if commons_room.is_some() && !active_carries_room && carrier_idx.is_none() {
        state.ws_status =
            "No connected server carries this Commons room right now.".to_string();
        return false; // keep the draft in the input box
    }
    // Read-only Commons rooms refuse sends for non-mods, whichever carrier
    // would relay them (field test 5) -- same rule the room enforces at home.
    if let Some(room) = commons_room.as_deref() {
        if commons_room_read_only(state, room) == Some(true) {
            state.ws_status = format!("#{} is read-only.", room);
            return false; // keep the draft
        }
    }

    // Sealed-sender DM prep (v2, 2026-08-23): build the two puts and
    // persist our copy to the LOCAL store BEFORE the ws_client borrow
    // below (the store insert mutates state). FAIL CLOSED: a DM that
    // can't be sealed is never sent in any form — the v2 protocol has no
    // plaintext field to downgrade to.
    let mut dm_prepared: Option<(String, String)> = None;
    if !is_p2p_group && !is_scratchpad && carrier_idx.is_none() && channel.starts_with("dm:") {
        if !state.ws_client.as_ref().map_or(false, |c| c.is_connected()) {
            state.ws_status = "Not connected; DM not sent.".to_string();
            return false; // keep the draft
        }
        let partner_key = channel[3..].to_string();
        match build_dm_puts(state, &partner_key, content, ts) {
            Ok((recipient_put, self_put, inner)) => {
                crate::engine::dm::ensure_dm_store(state);
                if let Some(store) = state.dm_store.as_mut() {
                    store.insert(&inner);
                    store.mark_read(&partner_key, inner.ts);
                    store.save();
                }
                dm_prepared = Some((recipient_put, self_put));
            }
            Err(reason) => {
                let human = match reason {
                    "no_own_key" => "Can't send: your identity isn't unlocked on this device. Recover from your seed phrase first.",
                    "missing_peer_key" => "Can't send yet: we don't have this person's encryption key. It arrives when they next come online; try again then.",
                    "bad_own_key" => "Can't send: your encryption key could not be derived. Try Identity → Recover.",
                    _ => "Can't send: encrypting the message failed.",
                };
                // Surface the reason INSIDE the conversation so it can't
                // be missed, and keep the draft in the composer.
                state.chat_messages.push(ChatMessage {
                    sender_name: "System".to_string(),
                    sender_key: String::new(),
                    content: human.to_string(),
                    timestamp: chrono_now_str(),
                    timestamp_ms: ts,
                    channel: channel.clone(),
                    server: norm_server_url(&state.server_url),
                    ..Default::default()
                });
                log::warn!("DM not sent ({reason}); fail closed, no plaintext path exists.");
                return false;
            }
        }
    }

    if !is_p2p_group && !is_scratchpad && carrier_idx.is_none() {
        if let Some(ref client) = state.ws_client {
            if client.is_connected() {
                let display_name = if !state.user_name.is_empty() {
                    state.user_name.clone()
                } else if let Some(me) = state.chat_users.iter().find(|u| u.public_key == state.profile_public_key) {
                    if !me.name.is_empty() && me.name != "Anonymous" { me.name.clone() } else { "Anonymous".to_string() }
                } else {
                    "Anonymous".to_string()
                };

                // DM: sealed-sender v2, FAIL CLOSED. The envelopes were
                // built and locally persisted in the prep block above;
                // here we just put them on the wire. Self-copy inline,
                // recipient copy through the standard send below.
                let json_str_opt: Option<String> = if channel.starts_with("dm:") {
                    match dm_prepared.take() {
                        Some((recipient_put, self_put)) => {
                            crate::debug::push_debug(format!("WS >>> {}", self_put));
                            client.send(&self_put);
                            Some(recipient_put)
                        }
                        None => None,
                    }
                } else if channel.starts_with("group:") {
                    let group_id = &channel[6..];
                    Some(serde_json::json!({
                        "type": "group_msg",
                        "group_id": group_id,
                        "content": content,
                    }).to_string())
                } else {
                    // Normal channel chat, Dilithium-signed; carries reply_to
                    // when a reply context is active. A Commons view sends
                    // into the bare room name (wire_channel).
                    let mut chat_obj = serde_json::json!({
                        "type": "chat",
                        "from": state.profile_public_key,
                        "from_name": display_name,
                        "content": content,
                        "timestamp": ts,
                        "channel": wire_channel,
                    });
                    if let Some(ref r) = state.chat_reply_to {
                        chat_obj["reply_to"] = serde_json::json!({
                            "from": r.sender_key,
                            "from_name": r.sender_name,
                            "content": r.preview,
                            "timestamp": r.timestamp_ms,
                        });
                    }
                    if let Some(seed) = state.private_key_bytes.as_ref() {
                        chat_obj["pq_signature"] = serde_json::Value::String(
                            crate::net::identity::pq_sign_chat(seed, content, ts)
                        );
                    }
                    Some(chat_obj.to_string())
                };
                if let Some(json_str) = json_str_opt {
                    crate::debug::push_debug(format!("WS >>> {}", json_str));
                    client.send(&json_str);
                    state.chat_sent_timestamps.push(ts);
                    if state.chat_sent_timestamps.len() > 20 {
                        state.chat_sent_timestamps.remove(0);
                    }
                } else {
                    send_aborted = true;
                }
            }
        }
    }

    // Commons carrier send: same signed chat message, different socket.
    // The echo goes into the CARRIER's buffer so the merged Commons view
    // shows it exactly once (the default active-buffer echo is skipped).
    if let Some(ci) = carrier_idx {
        let display_name = if !state.user_name.is_empty() {
            state.user_name.clone()
        } else {
            "Anonymous".to_string()
        };
        let mut chat_obj = serde_json::json!({
            "type": "chat",
            "from": state.profile_public_key,
            "from_name": display_name,
            "content": content,
            "timestamp": ts,
            "channel": wire_channel,
        });
        if let Some(ref r) = state.chat_reply_to {
            chat_obj["reply_to"] = serde_json::json!({
                "from": r.sender_key,
                "from_name": r.sender_name,
                "content": r.preview,
                "timestamp": r.timestamp_ms,
            });
        }
        if let Some(seed) = state.private_key_bytes.as_ref() {
            chat_obj["pq_signature"] = serde_json::Value::String(
                crate::net::identity::pq_sign_chat(seed, content, ts),
            );
        }
        let json_str = chat_obj.to_string();
        let my_key = state.profile_public_key.clone();
        let local_reply = state.chat_reply_to.clone();
        let now = chrono_now_str();
        let conn = &mut state.connections[ci];
        if let Some(ws) = conn.ws.as_ref() {
            crate::debug::push_debug(format!("WS(bg) >>> {}", json_str));
            ws.send(&json_str);
        }
        conn.sent_timestamps.push(ts);
        if conn.sent_timestamps.len() > 20 {
            conn.sent_timestamps.remove(0);
        }
        let echo_server = conn.url.clone();
        conn.messages.push(ChatMessage {
            sender_name: display_name,
            sender_key: my_key,
            content: content.to_string(),
            timestamp: now,
            timestamp_ms: ts,
            channel: wire_channel.clone(),
            reply_to: local_reply,
            server: echo_server,
            ..Default::default()
        });
        while conn.messages.len() > 200 {
            conn.messages.remove(0);
        }
        state.chat_reply_to = None;
        return true;
    }

    if send_aborted {
        return false;
    }

    // Local echo so the sender sees their message immediately.
    let now = chrono_now_str();
    let local_name = if !state.user_name.is_empty() {
        state.user_name.clone()
    } else if let Some(me) = state.chat_users.iter().find(|u| u.public_key == state.profile_public_key) {
        if !me.name.is_empty() && me.name != "Anonymous" { me.name.clone() } else { "You".to_string() }
    } else {
        "You".to_string()
    };
    let local_reply_to = state.chat_reply_to.clone();
    // Keep the DM sidebar preview current for our own sends (v0.715).
    if let Some(pk) = channel.strip_prefix("dm:") {
        if let Some(d) = state.chat_dms.iter_mut().find(|d| d.user_key == pk) {
            d.last_message = format!("You: {}", content);
            d.timestamp = now.clone();
            d.unread = false;
        }
    }
    state.chat_messages.push(ChatMessage {
        sender_name: local_name,
        sender_key: state.profile_public_key.clone(),
        content: content.to_string(),
        timestamp: now,
        timestamp_ms: ts,
        // A Commons send echoes at the bare room name so the merged
        // carrier view (which filters by room) shows it.
        channel: wire_channel,
        reply_to: local_reply_to,
        server: norm_server_url(&state.server_url),
        ..Default::default()
    });
    state.chat_reply_to = None;
    while state.chat_messages.len() > 200 {
        state.chat_messages.remove(0);
    }
    true
}

/// File types the chat attach picker offers -- mirrors the web client's
/// accept list (web/chat/index.html #file-upload-input).
pub(crate) const ATTACH_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "pdf", "txt", "md", "json",
    "zip", "tar.gz", "mp3", "mp4", "webm", "ogg", "wav",
    "blend", "stl", "obj", "gltf", "glb",
];

/// 3D/model formats auto-publish to the public Shared Files library
/// (upload with ?share=1), exactly like the web client.
const SHARE_EXTS: &[&str] = &["blend", "stl", "obj", "gltf", "glb"];

/// The real upload ceiling: nginx caps /api/upload bodies at 6 MB
/// (client_max_body_size 6m in scripts/nginx/humanity.conf).
pub(crate) const ATTACH_MAX_BYTES: u64 = 6 * 1024 * 1024;

/// Best-effort MIME from the filename extension (server re-checks anyway).
fn mime_for_filename(name: &str) -> &'static str {
    let lower = name.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "txt" | "md" => "text/plain",
        "json" => "application/json",
        "zip" => "application/zip",
        "gz" => "application/gzip",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

// ─────────────────────────────── UI Helpers ──────────────────────────────

/// Draw a lock/unlock toggle button — designed to sit flush in a panel
/// corner with NO surrounding padding. Allocates exactly 14×14 with no
/// horizontal wrapper or item spacing, so when the caller positions an
/// Area at the panel boundary the button paints exactly there.
/// Returns true if the button was clicked (toggle lock state).
fn draw_panel_lock_button(ui: &mut egui::Ui, theme: &Theme, locked: bool) -> bool {
    let tooltip = if locked { "Unlock panel width" } else { "Lock panel width" };
    // Locked reads as an active "held" state (warning token); unlocked is a quiet
    // muted glyph. Both from tokens so the lock affordance restyles with the app.
    let color = if locked { theme.warning() } else { theme.text_muted() };
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
fn tinted_section_header(ui: &mut egui::Ui, theme: &Theme, label: &str, collapsed: bool, bg: Color32) -> bool {
    tinted_section_header_with_buttons(ui, theme, label, collapsed, bg, |_| {})
}

/// Draw a tinted section header with optional right-aligned buttons.
/// The `add_buttons` closure receives the UI in right-to-left layout for adding icon buttons.
/// Returns true if the collapse arrow area was clicked (toggle collapse).
fn tinted_section_header_with_buttons(
    ui: &mut egui::Ui,
    theme: &Theme,
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
    let arrow_color = theme.text_secondary();
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
        egui::FontId::proportional(theme.font_size_small),
        theme.text_primary(),
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
fn section_header(ui: &mut egui::Ui, theme: &Theme, label: &str, collapsed: bool, bg: Color32) -> bool {
    let response = ui
        .allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 32.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                let full_rect = ui.max_rect();
                ui.painter().rect_filled(full_rect, 0.0, bg);
                ui.add_space(10.0);

                // Painted collapse/expand triangle
                let arrow_color = theme.text_secondary();
                let (arrow_rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), egui::Sense::hover());
                if collapsed {
                    crate::gui::widgets::icons::paint_triangle_right(ui.painter(), arrow_rect, arrow_color);
                } else {
                    crate::gui::widgets::icons::paint_triangle_down(ui.painter(), arrow_rect, arrow_color);
                }
                ui.label(
                    RichText::new(label)
                        .size(theme.font_size_small)
                        .color(theme.text_primary())
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

/// Normalize a server URL for identity comparison + provenance tagging
/// (trim whitespace, drop a trailing slash). This is the stable key used
/// by ChatMessage.server and, later, per-connection identity; every site
/// that compares or stores a server URL should pass it through here so
/// "https://x/" and "https://x" never read as two different servers.
pub(crate) fn norm_server_url(u: &str) -> String {
    u.trim().trim_end_matches('/').to_string()
}

/// One Commons room: a federated channel that TWO OR MORE of the user's
/// servers carry, so it renders once in the COMMONS sidebar section
/// instead of once per server (docs/design/federation-ux.md, "The
/// Commons"). Rooms are matched BY NAME across servers -- an accepted
/// limit until channels gain a portable identity; two unrelated servers
/// that both federate a "#general" will merge in this view.
pub(crate) struct CommonsRoom {
    /// Channel id/name (the wire channel string).
    pub name: String,
    /// How many of my servers carry it (active + background).
    pub carriers: usize,
    /// Whether the ACTIVE connection is one of the carriers (send path:
    /// active if possible, else a background carrier).
    pub active_carries: bool,
    /// Any carrier holds unread activity in this room.
    pub unread: bool,
    /// The carriers' urls, for the at-a-glance identity-color dots on the
    /// room row (field test 6: WHICH servers relay a room, without menus).
    pub carrier_urls: Vec<String>,
}

/// A server's stable identity color, derived from its normalized URL (FNV
/// hash to a hue). Shown as a chip on the server row and as matching dots
/// on the Commons rooms it carries -- the glanceable "who relays what".
pub(crate) fn server_color(url: &str) -> Color32 {
    let mut h: u32 = 2166136261;
    for b in norm_server_url(url).bytes() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    let hue = (h % 360) as f32 / 360.0;
    crate::gui::pages::escape_menu::hsv_to_rgb(hue, 0.55, 0.95)
}

/// The room name behind a Commons view id ("commons:general" -> "general"),
/// or None when the active channel is not a Commons view. A Commons room and
/// a same-named LOCAL channel are DIFFERENT rooms: the qualified id keeps
/// them apart (the operator's private #general must never blend into the
/// bridged #general, and vice versa).
pub(crate) fn commons_room_of(active_channel: &str) -> Option<&str> {
    active_channel.strip_prefix("commons:")
}

/// Open a Commons room: qualified view id, fresh buffer, history refetch,
/// and the unread mark cleared on every carrier.
pub(crate) fn open_commons_room(state: &mut GuiState, name: &str) {
    state.chat_active_channel = format!("commons:{}", name);
    state.chat_messages.clear();
    state.history_fetched = false;
    if let Some(c) = state.chat_channels.iter_mut().find(|c| c.id == name) {
        c.unread = false;
    }
    for conn in state.connections.iter_mut() {
        if let Some(c) = conn.channels.iter_mut().find(|c| c.id == name) {
            c.unread = false;
        }
    }
}

/// Whether the resolved Commons send carrier treats `room` as read-only for
/// THIS user (admins/mods still post -- e.g. #announcements). None = no
/// connected carrier at all. Field test 5: bridged read-only rooms must not
/// accept posts through the side door of a different carrier.
pub(crate) fn commons_room_read_only(state: &GuiState, room: &str) -> Option<bool> {
    let is_mod = |role: &str| role == "admin" || role == "owner" || role == "moderator" || role == "mod";
    let active_carries = state.ws_client.as_ref().map_or(false, |c| c.is_connected())
        && state.chat_channels.iter().any(|c| c.id == room && c.federated);
    if active_carries {
        let ro = state
            .chat_channels
            .iter()
            .find(|c| c.id == room)
            .map(|c| c.read_only)
            .unwrap_or(false);
        return Some(ro && !is_mod(&viewer_role(state)));
    }
    let conn = state.connections.iter().find(|c| {
        c.identified
            && c.ws.is_some()
            && c.channels.iter().any(|ch| ch.id == room && ch.federated)
    })?;
    let ro = conn
        .channels
        .iter()
        .find(|ch| ch.id == room)
        .map(|ch| ch.read_only)
        .unwrap_or(false);
    let role = conn
        .users
        .iter()
        .find(|u| u.public_key == state.profile_public_key)
        .map(|u| u.role.clone())
        .unwrap_or_default();
    Some(ro && !is_mod(&role))
}

/// Compute the Commons rooms from the active connection + every background
/// connection. A room qualifies when it is FLAGGED federated and at least
/// two of my servers carry it.
pub(crate) fn commons_rooms(state: &GuiState) -> Vec<CommonsRoom> {
    use std::collections::BTreeMap;
    // name -> (carriers, active_carries, unread, carrier_urls)
    let mut rooms: BTreeMap<String, (usize, bool, bool, Vec<String>)> = BTreeMap::new();
    let active_connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    if active_connected {
        let active_url = norm_server_url(&state.server_url);
        for ch in state.chat_channels.iter().filter(|c| c.federated) {
            let e = rooms.entry(ch.id.clone()).or_insert((0, false, false, Vec::new()));
            e.0 += 1;
            e.1 = true;
            e.2 |= ch.unread;
            e.3.push(active_url.clone());
        }
    }
    for conn in state.connections.iter().filter(|c| c.identified) {
        for ch in conn.channels.iter().filter(|c| c.federated) {
            let e = rooms.entry(ch.id.clone()).or_insert((0, false, false, Vec::new()));
            e.0 += 1;
            e.2 |= ch.unread;
            e.3.push(conn.url.clone());
        }
    }
    rooms
        .into_iter()
        .filter(|(_, (carriers, _, _, _))| *carriers >= 2)
        .map(|(name, (carriers, active_carries, unread, carrier_urls))| CommonsRoom {
            name,
            carriers,
            active_carries,
            unread,
            carrier_urls,
        })
        .collect()
}

/// The COMMONS sidebar section: federated rooms carried by 2+ of my
/// servers, each rendered ONCE. Clicking opens the room exactly like a
/// normal channel (the center view then merges every carrier's copy).
fn draw_commons_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let rooms = commons_rooms(state);
    if rooms.is_empty() {
        return; // no bridged rooms, no section -- zero noise for solo servers
    }
    let collapsed = state.chat_commons_collapsed;
    let mut info_clicked = false;
    if tinted_section_header_with_buttons(
        ui,
        theme,
        &format!("Commons ({})", rooms.len()),
        collapsed,
        theme.server_bg(),
        |ui| {
            // "?" opens the full explainer (field test 4): what the
            // Commons is, how bridging works, what stays local.
            let (q_rect, q_resp) = crate::gui::widgets::icons::icon_button(ui, 14.0);
            let q_color = if q_resp.hovered() { Color32::WHITE } else { theme.text_secondary() };
            ui.painter().text(
                q_rect.center(),
                egui::Align2::CENTER_CENTER,
                "?",
                egui::FontId::proportional(theme.font_size_small),
                q_color,
            );
            if q_resp.on_hover_text("What is the Commons?").clicked() {
                info_clicked = true;
            }
        },
    ) {
        state.chat_commons_collapsed = !state.chat_commons_collapsed;
    }
    if info_clicked {
        state.show_commons_info = true;
    }
    draw_commons_info_modal(ui.ctx(), theme, state);
    if collapsed {
        return;
    }
    Frame::NONE
        .fill(theme.server_bg())
        .inner_margin(egui::Margin::symmetric(0, 1))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            for room in &rooms {
                let is_active = commons_room_of(&state.chat_active_channel) == Some(room.name.as_str());
                let row_w = ui.available_width();
                let (row_rect, response) = ui.allocate_exact_size(
                    Vec2::new(row_w, theme.row_height * 0.9),
                    egui::Sense::click(),
                );
                if !ui.is_rect_visible(row_rect) {
                    continue;
                }
                // Active = accent wash + left bar, hover = the shared surface
                // token (mirrors the scratchpad row's theme-following states).
                let acc = theme.accent();
                if is_active {
                    ui.painter().rect_filled(
                        row_rect,
                        0.0,
                        Color32::from_rgba_unmultiplied(acc.r(), acc.g(), acc.b(), 60),
                    );
                    let bar = egui::Rect::from_min_size(
                        row_rect.min,
                        Vec2::new(3.0, row_rect.height()),
                    );
                    ui.painter().rect_filled(bar, 0.0, acc);
                } else if response.hovered() {
                    ui.painter().rect_filled(row_rect, 0.0, theme.bg_tertiary());
                }
                let cy = row_rect.center().y;
                let mut cx = row_rect.left() + theme.item_padding + 2.0;
                // Bridge badge: the both-ways arrow marks a federated room.
                let badge = egui::Rect::from_min_size(
                    egui::pos2(cx, cy - 5.0),
                    Vec2::new(12.0, 10.0),
                );
                crate::gui::widgets::icons::paint_arrow_both(
                    ui.painter(),
                    badge,
                    if is_active { theme.accent() } else { theme.text_muted() },
                );
                cx += 16.0;
                let text_color = if is_active {
                    theme.text_primary()
                } else {
                    theme.text_secondary()
                };
                let name_str = format!("# {}", room.name);
                ui.painter().text(
                    egui::pos2(cx, cy),
                    egui::Align2::LEFT_CENTER,
                    &name_str,
                    egui::FontId::proportional(theme.font_size_body),
                    text_color,
                );
                // Carrier identity dots (field test 6): WHICH of my servers
                // relay this room, at a glance -- each dot matches the color
                // chip on that server's row. Hover names them.
                {
                    let name_w = ui.fonts(|f| {
                        f.layout_no_wrap(
                            name_str.clone(),
                            egui::FontId::proportional(theme.font_size_body),
                            text_color,
                        )
                    })
                    .size()
                    .x;
                    let mut dx = cx + name_w + 10.0;
                    for cu in room.carrier_urls.iter().take(6) {
                        ui.painter().circle_filled(
                            egui::pos2(dx, cy),
                            3.5,
                            server_color(cu),
                        );
                        dx += 10.0;
                    }
                    let dots_rect = egui::Rect::from_min_max(
                        egui::pos2(cx + name_w + 4.0, row_rect.top()),
                        egui::pos2(dx, row_rect.bottom()),
                    );
                    let dots_resp = ui.interact(
                        dots_rect,
                        ui.id().with(("commons_dots", &room.name)),
                        egui::Sense::hover(),
                    );
                    dots_resp.on_hover_text(format!(
                        "Relayed by: {}",
                        room.carrier_urls
                            .iter()
                            .map(|u| server_display_name(u))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                // Right edge: unread dot, else the carrier count.
                if room.unread && !is_active {
                    ui.painter().circle_filled(
                        egui::pos2(row_rect.right() - 12.0, cy),
                        3.0,
                        theme.accent(),
                    );
                } else {
                    ui.painter().text(
                        egui::pos2(row_rect.right() - 8.0, cy),
                        egui::Align2::RIGHT_CENTER,
                        format!("{}", room.carriers),
                        egui::FontId::proportional(theme.font_size_small),
                        theme.text_muted(),
                    );
                }
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    response.clone().on_hover_text(format!(
                        "Bridged across {} of your servers",
                        room.carriers
                    ));
                }
                if response.clicked() && !is_active {
                    open_commons_room(state, &room.name);
                }
            }
        });
}

/// Extract a display name from a server URL.
pub(crate) fn server_display_name(url: &str) -> String {
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
            let display_emoji: String = emoji.chars().filter(|c| *c != '\u{FE0F}').collect(); // glyph-exempt: stripping the selector, not rendering it
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
            let display_emoji: String = emoji.chars().filter(|c| *c != '\u{FE0F}').collect(); // glyph-exempt: stripping the selector, not rendering it
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
    } else if let Some(room) = commons_room_of(active_channel) {
        // A Commons VIEW id is not a storable channel; file the notice
        // under the underlying room so it renders in the merged view
        // (when the active server carries it) and is never orphaned.
        room.to_string()
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

/// Blocking upload of a clipboard PNG -- thin wrapper over
/// `upload_file_blocking` (kept for the existing paste call sites).
fn upload_image_png_blocking(
    server_url: &str,
    public_key: &str,
    png_bytes: Vec<u8>,
) -> Result<String, String> {
    upload_file_blocking(server_url, public_key, "clipboard.png", "image/png", png_bytes, false)
}

/// Blocking multipart upload of any file to `<server_url>/api/upload?key=<pk>`
/// (v0.708; `share=true` adds `&share=1` so 3D/model files publish to the
/// public Shared Files library, matching the web client). Returns the URL
/// from the JSON response. Runs on a worker thread at every call site, so
/// blocking here never freezes a frame. nginx caps the body at 6 MB.
pub(crate) fn upload_file_blocking(
    server_url: &str,
    public_key: &str,
    filename: &str,
    mime: &str,
    bytes: Vec<u8>,
    share: bool,
) -> Result<String, String> {
    upload_file_blocking_ext(server_url, public_key, filename, mime, bytes, share, false)
}

/// Prepare an attachment for sending and return the message CONTENT string to
/// route through the normal send path (2026-08-24). For a DM the file is
/// encrypted client-side, the ciphertext is uploaded, and a FILE_MARKER
/// (carrying the key inside the sealed envelope) is returned. For a public
/// channel the plain file is uploaded and its URL is returned. Runs on the
/// upload worker thread.
pub(crate) fn prepare_attachment_send(
    server_url: &str,
    public_key: &str,
    filename: &str,
    mime: &str,
    bytes: Vec<u8>,
    share: bool,
    is_dm: bool,
) -> Result<String, String> {
    if is_dm {
        let size = bytes.len() as u64;
        let (ciphertext, k, n) = crate::net::dm_pq::encrypt_attachment(&bytes)?;
        let url = upload_file_blocking_ext(
            server_url, public_key, "attachment.enc", "application/octet-stream",
            ciphertext, false, true,
        )?;
        Ok(crate::net::dm_pq::build_file_marker(&crate::net::dm_pq::DmAttachment {
            url,
            k,
            n,
            name: filename.to_string(),
            mime: mime.to_string(),
            size,
        }))
    } else {
        upload_file_blocking(server_url, public_key, filename, mime, bytes, share)
    }
}

/// As `upload_file_blocking`, plus an `encrypted` flag. When set, the body is
/// opaque ciphertext and the server skips format/EXIF handling
/// (`?encrypted=1`). Used for private DM attachments (2026-08-24).
pub(crate) fn upload_file_blocking_ext(
    server_url: &str,
    public_key: &str,
    filename: &str,
    mime: &str,
    bytes: Vec<u8>,
    share: bool,
    encrypted: bool,
) -> Result<String, String> {
    let base = server_url.trim_end_matches('/');
    let share_q = if share { "&share=1" } else { "" };
    let enc_q = if encrypted { "&encrypted=1" } else { "" };
    let upload_url = format!("{base}/api/upload?key={key}{share_q}{enc_q}", base = base, key = public_key);
    let boundary = format!("HumanityOSBoundary{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    // Sanitize the filename for the multipart header (quotes/CRLF would
    // corrupt the form-data framing).
    let safe_name: String = filename
        .chars()
        .map(|c| if c == '"' || c == '\r' || c == '\n' { '_' } else { c })
        .collect();
    let preamble = format!(
        "--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{f}\"\r\nContent-Type: {m}\r\n\r\n",
        b = boundary, f = safe_name, m = mime,
    );
    let epilogue = format!("\r\n--{b}--\r\n", b = boundary);
    let mut body: Vec<u8> = Vec::with_capacity(preamble.len() + bytes.len() + epilogue.len());
    body.extend_from_slice(preamble.as_bytes());
    body.extend_from_slice(&bytes);
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

pub(crate) fn name_color(name: &str) -> Color32 {
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

// ──────────────────────────────────────────────────────────────────────────
// Sealed-sender DMs (v2, 2026-08-23). FAIL CLOSED, no plaintext path.
// ──────────────────────────────────────────────────────────────────────────

/// Build the two sealed-sender puts for one DM: the SAME Dilithium-signed
/// inner payload sealed once to the recipient (their mailbox) and once to
/// ourselves (our mailbox, so our other devices can fetch sent history).
/// The relay stores each without any sender column.
///
/// Returns `(recipient_put_json, self_put_json, verified_inner)`.
///
/// Failure reasons (surfaced in the DM view; there is deliberately NO
/// "send unencrypted anyway" fallback — the v2 protocol has no plaintext
/// field to fall back to):
///   - `"no_own_key"`        — the BIP39 seed isn't unlocked on this device
///   - `"missing_peer_key"`  — recipient's Kyber768 public key isn't known
///   - `"bad_own_key"`       — keypair derivation failed
///   - `"encryption_failed"` — signing or sealing errored
fn build_dm_puts(
    state: &GuiState,
    partner_key: &str,
    content: &str,
    ts: u64,
) -> Result<(String, String, crate::net::dm_pq::DmInner), &'static str> {
    let seed = state.private_key_bytes.as_ref().ok_or("no_own_key")?;
    let my_kp = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(seed)
        .map_err(|_| "bad_own_key")?;
    let peer_kyber = state.peer_kyber_keys.get(partner_key)
        .ok_or("missing_peer_key")?;
    let inner_json = crate::net::dm_pq::build_signed_inner(
        seed, &state.profile_public_key, partner_key, ts, content,
    )
    .map_err(|_| "encryption_failed")?;
    let inner = crate::net::dm_pq::parse_verify_inner(&inner_json)
        .map_err(|_| "encryption_failed")?;
    let env_recipient = crate::net::dm_pq::seal_v2(peer_kyber, &inner_json)
        .map_err(|_| "encryption_failed")?;
    let env_self = crate::net::dm_pq::seal_v2(&my_kp.public_base64(), &inner_json)
        .map_err(|_| "encryption_failed")?;
    // Attach the recipient's friendship certificate when we hold one
    // (follows removal 2026-08-24): certified mail rides the friend lane;
    // without it this send spends the sender's daily knock budget.
    let mut put_recipient = serde_json::json!({
        "type": "dm_put", "to": partner_key, "content": env_recipient,
    });
    if let Some(cert) = state.dm_store.as_ref().and_then(|s| s.cert_for(partner_key)) {
        put_recipient["friend_cert"] = serde_json::Value::String(cert.to_string());
    }
    let put_self =
        serde_json::json!({ "type": "dm_put", "to": state.profile_public_key, "content": env_self })
            .to_string();
    Ok((put_recipient.to_string(), put_self, inner))
}

/// Open a DM conversation: switch the channel and load its history from
/// the LOCAL encrypted store — the relay keeps no DM history any more
/// (its mailbox is a sender-less delivery window that expires).
pub(crate) fn open_dm_conversation(state: &mut GuiState, key: &str) {
    state.chat_active_channel = format!("dm:{key}");
    state.chat_messages.clear();
    state.history_fetched = false;
    crate::engine::dm::ensure_dm_store(state);
    if let Some(store) = state.dm_store.as_mut() {
        if let Some(last) = store.conversation(key).last().map(|m| m.ts) {
            store.mark_read(key, last);
        }
    }
    if let Some(d) = state.chat_dms.iter_mut().find(|d| d.user_key == key) {
        d.unread = false;
    }
    crate::engine::dm::reload_dm_channel(state, key);
}

#[cfg(test)]
mod code_block_tests {
    use super::extract_code_blocks;

    #[test]
    fn no_fence_is_untouched() {
        let (t, b) = extract_code_blocks("just a plain message with `inline` code");
        assert_eq!(t, "just a plain message with `inline` code");
        assert!(b.is_empty());
    }

    #[test]
    fn a_fenced_block_with_language_is_extracted() {
        let (t, b) = extract_code_blocks("before\n```rust\nlet x = 1;\n```\nafter");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].lang, "rust");
        assert_eq!(b[0].code, "let x = 1;");
        // The surrounding prose survives; the fence is gone.
        assert!(t.contains("before"));
        assert!(t.contains("after"));
        assert!(!t.contains("```"));
        assert!(!t.contains("let x = 1;"));
    }

    #[test]
    fn a_fence_without_language_keeps_the_first_line_as_code() {
        let (_t, b) = extract_code_blocks("```\nplain code\nline two\n```");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].lang, "");
        assert_eq!(b[0].code, "plain code\nline two");
    }

    #[test]
    fn language_token_is_the_leading_word_chars_matching_web() {
        // Mirrors web's /```(\w*)\n?.../: "echo" is word chars so it's the lang;
        // the space stops the token and " hello\nworld" is the code (one optional
        // newline after the lang would have been consumed, but here it's a space).
        let (_t, b) = extract_code_blocks("```echo hello\nworld\n```");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].lang, "echo");
        assert_eq!(b[0].code, " hello\nworld");
    }

    #[test]
    fn unclosed_fence_is_left_verbatim() {
        let (t, b) = extract_code_blocks("look: ```rust\nlet x = 1; (never closed)");
        assert!(b.is_empty());
        assert_eq!(t, "look: ```rust\nlet x = 1; (never closed)");
    }

    #[test]
    fn two_blocks_are_both_extracted_in_order() {
        let (_t, b) = extract_code_blocks("```py\na\n```\nmid\n```js\nb\n```");
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].lang, "py");
        assert_eq!(b[0].code, "a");
        assert_eq!(b[1].lang, "js");
        assert_eq!(b[1].code, "b");
    }
}

#[cfg(test)]
mod bulletize_tests {
    use super::bulletize_list_lines;

    #[test]
    fn plain_message_is_untouched() {
        assert_eq!(bulletize_list_lines("hello there"), "hello there");
        assert_eq!(bulletize_list_lines("line one\nline two"), "line one\nline two");
    }

    #[test]
    fn dash_and_star_line_starts_become_a_bullet() {
        assert_eq!(bulletize_list_lines("- apples\n- pears"), "\u{2022} apples\n\u{2022} pears");
        assert_eq!(bulletize_list_lines("* one"), "\u{2022} one");
    }

    #[test]
    fn leading_indentation_is_preserved() {
        assert_eq!(bulletize_list_lines("  - nested"), "  \u{2022} nested");
    }

    #[test]
    fn inline_emphasis_and_mid_line_dashes_are_left_alone() {
        // No trailing space after the leading `*` => an italic run, not a list.
        assert_eq!(bulletize_list_lines("*italic* text"), "*italic* text");
        // A dash that isn't at the line start is prose, not a bullet.
        assert_eq!(bulletize_list_lines("a - b"), "a - b");
        // A bare marker with no following space is left as-is.
        assert_eq!(bulletize_list_lines("-x"), "-x");
    }

    #[test]
    fn substitution_is_width_preserving_so_inline_spans_stay_aligned() {
        // The whole point: "- " (2 chars) -> "\u{2022} " (2 chars), so a bold
        // run later on the same line keeps its char offsets. The char count of
        // each line must be identical before and after.
        let src = "- buy *milk* today";
        let out = bulletize_list_lines(src);
        assert_eq!(src.chars().count(), out.chars().count());
        assert!(out.starts_with("\u{2022} buy *milk* today"));
    }
}

#[cfg(test)]
mod commons_rooms_tests {
    use super::commons_rooms;
    use crate::gui::{ChatChannel, GuiState, ServerConnection};

    fn conn(url: &str, identified: bool, channels: &[(&str, bool, bool)]) -> ServerConnection {
        ServerConnection {
            url: url.to_string(),
            identified,
            channels: channels
                .iter()
                .map(|(id, federated, unread)| ChatChannel {
                    id: id.to_string(),
                    name: id.to_string(),
                    federated: *federated,
                    unread: *unread,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn a_room_two_servers_bridge_is_commons_and_one_server_is_not() {
        let mut state = GuiState::default();
        // Active connection is offline (no ws), so carriers come from the
        // two background connections.
        state.connections.push(conn("https://a", true, &[("general", true, false), ("ops", true, false)]));
        state.connections.push(conn("https://b", true, &[("general", true, true), ("lounge", false, false)]));
        let rooms = commons_rooms(&state);
        assert_eq!(rooms.len(), 1, "only the room BOTH servers federate qualifies");
        assert_eq!(rooms[0].name, "general");
        assert_eq!(rooms[0].carriers, 2);
        assert!(!rooms[0].active_carries, "active connection is offline");
        assert!(rooms[0].unread, "unread on ANY carrier lights the room");
    }

    #[test]
    fn same_name_without_the_federated_flag_never_merges() {
        let mut state = GuiState::default();
        state.connections.push(conn("https://a", true, &[("general", false, false)]));
        state.connections.push(conn("https://b", true, &[("general", false, false)]));
        assert!(commons_rooms(&state).is_empty(), "two plain #general rooms stay separate");
    }

    #[test]
    fn opening_a_commons_room_uses_the_qualified_id_and_clears_carrier_unread() {
        let mut state = GuiState::default();
        state.connections.push(conn("https://a", true, &[("general", true, true)]));
        state.connections.push(conn("https://b", true, &[("general", true, true)]));
        super::open_commons_room(&mut state, "general");
        // The qualified id keeps the bridged room distinct from any
        // same-named LOCAL channel (the private #general must never blend
        // into the Commons view).
        assert_eq!(state.chat_active_channel, "commons:general");
        assert_eq!(super::commons_room_of(&state.chat_active_channel), Some("general"));
        assert!(
            state
                .connections
                .iter()
                .all(|c| c.channels.iter().all(|ch| !ch.unread)),
            "opening the room clears unread on every carrier"
        );
    }

    #[test]
    fn unidentified_connections_do_not_count_as_carriers() {
        let mut state = GuiState::default();
        state.connections.push(conn("https://a", true, &[("general", true, false)]));
        state.connections.push(conn("https://b", false, &[("general", true, false)]));
        assert!(commons_rooms(&state).is_empty(), "a half-connected server is not a carrier yet");
    }
}


#[cfg(test)]
mod ingame_chat_mode_tests {
    use super::dm_recency_order;
    use crate::gui::IngameChatMode;

    #[test]
    fn mode_cycle_visits_every_mode_and_wraps() {
        // Walking next() from Channels must visit all 4 modes exactly once
        // and land back on Channels -- the tab row, a future cycle hotkey,
        // and this test all share the single ALL ordering.
        let mut m = IngameChatMode::Channels;
        let mut seen = Vec::new();
        for _ in 0..IngameChatMode::ALL.len() {
            seen.push(m);
            m = m.next();
        }
        assert_eq!(seen, IngameChatMode::ALL.to_vec());
        assert_eq!(m, IngameChatMode::Channels); // wrapped
    }

    #[test]
    fn mode_labels_are_distinct_and_nonempty() {
        let labels: Vec<&str> = IngameChatMode::ALL.iter().map(|m| m.label()).collect();
        for l in &labels {
            assert!(!l.is_empty());
        }
        let unique: std::collections::HashSet<&&str> = labels.iter().collect();
        assert_eq!(unique.len(), labels.len(), "tab labels must not collide");
    }

    #[test]
    fn dm_recency_most_recent_first_hour_min_format() {
        // Same-format display strings ("HH:MM UTC") order lexicographically;
        // descending = most recent first.
        let ts = ["09:15 UTC", "17:42 UTC", "12:00 UTC"];
        assert_eq!(dm_recency_order(&ts), vec![1, 2, 0]);
    }

    #[test]
    fn dm_recency_most_recent_first_date_format() {
        // ISO date-bearing strings also order lexicographically.
        let ts = [
            "2026-05-29 17:42 UTC",
            "2026-07-10 08:00 UTC",
            "2026-01-02 23:59 UTC",
        ];
        assert_eq!(dm_recency_order(&ts), vec![1, 0, 2]);
    }

    #[test]
    fn dm_recency_empty_timestamps_sink_to_end() {
        // A conversation opened but never messaged has no timestamp -- it must
        // trail every dated conversation, never lead the list.
        let ts = ["", "17:42 UTC", "", "09:15 UTC"];
        assert_eq!(dm_recency_order(&ts), vec![1, 3, 0, 2]);
    }

    #[test]
    fn dm_recency_ties_keep_incoming_order() {
        // Stable sort: equal timestamps (and the all-empty case) preserve the
        // store's existing order (the Chat page keeps it alphabetical).
        let ts = ["12:00 UTC", "12:00 UTC", "12:00 UTC"];
        assert_eq!(dm_recency_order(&ts), vec![0, 1, 2]);
        let empty = ["", "", ""];
        assert_eq!(dm_recency_order(&empty), vec![0, 1, 2]);
        assert_eq!(dm_recency_order(&[]), Vec::<usize>::new());
    }
}

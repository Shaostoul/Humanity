//! 3-panel chat page matching the website layout.
//!
//! LEFT:   Collapsible DMs (red), Groups (green), Servers (blue), Connection settings
//! MIDDLE: Channel header, message feed, input bar
//! RIGHT:  Friends list, Server members list

use super::chat_text::{bulletize_list_lines, draw_code_blocks, extract_code_blocks};
/// The HUD feed reads a message through the same parser (chat_text.rs).
pub(crate) use super::chat_text::one_line_formatted;
use egui::{Align, Color32, Frame, Layout, RichText, CornerRadius, ScrollArea, Stroke, Vec2};
use crate::gui::{ChatMessage, GuiState};
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
    drain_p2p_loaders, handle_p2p_group_obj,
    invite_to_group, join_group_with_ticket, leave_p2p_group, refresh_p2p_groups,
    spawn_group_load, spawn_groups_list_refresh,
};
use p2p_groups::{
    disband_p2p_group, mint_and_copy_p2p_invite, send_p2p_group_message,
};

/// The left rail: DMs, Groups, Commons, Servers and the expanded server's
/// channel list. See `chat/left_panel.rs`.
pub(crate) mod left_panel;
use left_panel::draw_left_panel;

/// The people surfaces: the Friends/Members right rail and the live strip.
/// See `chat/right_panel.rs`.
mod right_panel;
// Chat on a phone: one column at a time (Android rung A1, 2026-11-14).
mod phone;
use right_panel::{draw_live_strip, draw_right_panel};

/// The overlays drawn on top of the panels: pins, search, the user profile,
/// add-server, edit-channel, create/join group, the Commons explainer, the
/// slash-command help, and the voice call. See `chat/modals.rs`.
mod modals;
pub(crate) use modals::{create_p2p_group, draw_call_bar, draw_incoming_call_modal, draw_user_modal};
use modals::{
    draw_add_server_modal, draw_commons_info_modal, draw_create_group_modal,
    draw_edit_channel_modal, draw_help_modal, draw_join_group_modal, draw_pins_modal,
    draw_search_modal,
};

/// Who can reach me (step B): the refused-message notice and the Requests rail section.
/// See `chat/reach.rs`.
mod reach;
mod key_change; // key cards (2026-10-22): a changed key's notice with Accept, why a DM was not sent
/// Block (step C): the DM header's, the profile's and the member row's Block buttons.
mod blocking;
/// Report (step D): the Report dialog and its buttons. See `chat/report_dialog.rs`.
mod report_dialog;
pub(crate) use report_dialog::draw_report_dialog;
/// Calls through the server (step E): the line the call bar and the voice-room bar show when the
/// server cannot carry the call. See `chat/call_relay_bar.rs`.
mod call_relay_bar;
/// Warnings (step F): what is drawn under a received message. See `chat/warnings.rs`.
mod warnings;
/// The protected setup (step G): the rail's line, left-out rooms and pictures. See `chat/protected.rs`.
mod protected;
/// Joining and leaving a channel's voice room. See `chat/voice_room.rs`.
mod voice_room;
pub(crate) use voice_room::{leave_all_voice, set_voice_room, voice_wanted, LeftVoice};
/// Sending a file (10k): encrypted for a DM or a group, plain for a public channel. See `chat/attach_send.rs`.
pub(crate) mod attach_send;
/// A sent file's bytes kept to one copy: read on the worker, encrypted in place, streamed (round 3).
mod attach_body;
pub(crate) use attach_send::note_upload_limit; // the server tells each person their own upload limit (2026-10-10)
pub(crate) use attach_send::{room_limit_here, upload_token_here, UploadLimits, NOT_SIGNED_IN_YET};
/// The attach step: what is hidden in a file, the switch, Send anyway (2026-10-11). See `chat/attach_step.rs`.
pub(crate) mod attach_step;
pub use attach_step::FilesFlow;
/// Attach on a phone: Android's own document picker (2026-11-23). See `chat/attach_android.rs`.
/// Only an Android build opens it; on a desktop only its tests call it.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) mod attach_android;
/// Showing a private file (10k): the picture inline, or a card with Save. See `chat/attach_view.rs`.
mod attach_view;
/// Files posted into a public channel: a card with Download, and View in 3D for a model (2026-10-17).
/// See `chat/public_files.rs`.
pub(crate) mod public_files;
pub(crate) use public_files::{draw_model_window, post_in};
/// A posted video plays inside its card, one at a time, nothing fetched before Play (2026-11-06).
/// See `chat/inline_video.rs`.
pub(crate) mod inline_video;
pub(crate) use inline_video::{draw_video_window, sync_video_sound};
/// The dock a playing video moves into when its row leaves the view, following the person round the
/// app's pages (rung 3, 2026-11-06). See `chat/video_dock.rs`.
pub(crate) mod video_dock;
pub(crate) use video_dock::draw_video_dock;
/// A posted video's poster picture: made when it is posted, drawn before Play (2026-11-06). See
/// `chat/video_poster.rs`.
pub(crate) mod video_poster;

/// A chat video's "Volume settings" link was pressed (inline_video.rs, `ChatVideos::open_volume_settings`):
/// open Settings > Audio, where the choice of what volume a new video starts at is (2026-11-06). The
/// main loop asks after the video window each frame, and the page asks as it starts, which is what
/// catches a press on a wall screen showing the chat (it draws with its own `ChatVideos` swapped in).
pub(crate) fn follow_volume_settings_link(state: &mut GuiState) {
    if std::mem::take(&mut state.chat_video.open_volume_settings) {
        state.go_to(crate::gui::history::Place::settings(crate::gui::SettingsCategory::Audio), crate::gui::How::Beside);
    }
}
/// Device-to-device big files (answer 10): a row per transfer at the top of a DM. See `chat/transfer_rows.rs`.
mod transfer_rows;
#[cfg(test)]
pub(crate) use attach_view::{cache_key as private_file_key, open as open_private_file};
/// Quote / reply: a reply stays in its own conversation and never carries private words or a
/// file's key. See `chat/reply.rs`. A notice's text goes through its `preview_of` too
/// (engine/notify.rs), so a notice never holds a file's key either.
pub(crate) mod reply;
/// How much of a conversation's last message the DM list shows (2026-11-03). See `chat/dm_rows.rs`.
pub(crate) mod dm_rows;
/// THE room row of the sidebar, one drawer for a room under any server, active or not (BUG-242).
/// See `chat/channel_row.rs`.
mod channel_row;
// Links to channels (2026-11-03): Copy link, and a channel link opening here, not in the browser.
pub(crate) mod channel_link;
/// Mute server and Mute room, and a server row's muted mark (2026-10-30,
/// docs/design/notifications.md). See `chat/mute_menu.rs`.
mod mute_menu;
/// "Your settings for <name>" on another person's profile card (2026-11-03,
/// docs/design/per-person-controls.md). See `chat/person_controls.rs`.
mod person_controls;
// A line's length against the server's limit, and whether my line was posted (2026-10-24).
pub(crate) mod line_limit;
// Who can read and post in a channel: the line above the box, the pickers, plain links (2026-11-02).
pub(crate) mod access;
// The timestamp formats and formatters (moved out 2026-10-24, file-size ratchet).
mod timestamps;
pub use timestamps::*;
// The Commons: which of my LINKED servers make up a bridged room, and its routing (2026-10-24).
mod commons;
pub use commons::FederationLinks;
pub(crate) use commons::{
    commons_carriers, commons_room_of, commons_room_read_only, commons_rooms, commons_rooms_carried_by, commons_route, in_commons,
    open_commons_room, pump_federation_links, CommonsRoom,
};
// The message rows and the composer: one copy each, for the Chat page and the chat footer on
// the other pages (gui/bottom_strip/chat_footer.rs, 2026-11-15).
pub(crate) mod rows;
pub(crate) mod composer;
#[cfg(test)]
mod composer_tests; // the composer grows upward and stays on screen (2026-10-10)
#[cfg(test)]
mod channel_row_tests; // a room row is the same under every server (BUG-242)
#[cfg(test)]
mod self_mail_tests; // my copy of a DM leaves apart from the recipient's (BUG-259)
#[cfg(test)]
mod emoji_tests; // emoji pictures in reaction pills; Copy text gives the real emoji (2026-11-20)

// Maximum messages kept in the local chat buffer (was hardcoded, now uses theme.max_messages if needed).

/// Minimum panel width in points.
const MIN_PANEL_WIDTH: f32 = 150.0;
/// Maximum panel width in points.
const MAX_PANEL_WIDTH: f32 = 400.0;

// Section tint colors now come from theme.ron (theme.dm_bg(), theme.group_bg(), theme.server_bg(), etc.)

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    crate::engine::warnings::ensure_loaded(state); // step F's patterns, read once
    reply::drop_if_elsewhere(state); // a reply belongs to the conversation it was started in
    follow_volume_settings_link(state); // a video's "Volume settings" pressed last frame (a wall screen's too)
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
    // File-attach picker modal (v0.708). On pick: read it and upload it on a
    // worker thread (chat/attach_send.rs: encrypted for a DM or a group, 10k);
    // the finished upload drains below and routes via send_composed_content.
    if let Some(mut picker) = state.chat_attach_picker.take() {
        use crate::gui::widgets::file_browser::{file_picker_modal, FilePickerResult};
        match file_picker_modal(ctx, theme, &mut picker, "Attach files: choose one or more") {
            FilePickerResult::Open => state.chat_attach_picker = Some(picker),
            FilePickerResult::Cancelled => {}
            FilePickerResult::Picked(paths) => attach_send::start_attach_all(state, paths),
        }
    }
    // On a phone, Attach opens Android's own document picker instead (2026-11-23): its answer,
    // and what was chosen once read, arrive here (chat/attach_android.rs).
    #[cfg(target_os = "android")]
    attach_android::poll(ctx, state);
    attach_step::draw(ctx, theme, state); // the picked file's attach step, until Send or Cancel
    attach_send::offer_drop(ctx, state); // files dropped on the window go here (gui/drop_files.rs)
    crate::gui::pages::files::chat_card::apply(state); // what the Media cards asked last frame
    attach_send::next_queued(state); // then the next of several chosen together

    if std::mem::take(&mut state.pending_clipboard_paste) {
        // Grab the PNG on the main thread (clipboard access); the upload runs on a
        // worker thread. If there is no image, egui's TextEdit handles the text paste.
        if let Some(png_bytes) = try_grab_clipboard_image_as_png() {
            attach_send::start_paste(state, png_bytes);
        }
    }
    attach_send::drain(ctx, state); // a finished upload: its text goes out like typed text

    // ── LIVE STRIP (v0.1150, operator direction: "expandable tab at the top
    // of the chat page" for streaming). Declared BEFORE the side panels so
    // egui gives it the full window width above all three columns.
    draw_live_strip(ctx, theme, state);

    // The rooms, the open room and the people: three columns on a desktop, one at a time on a
    // phone (Android rung A1, chat/phone.rs).
    if humanity_core::platform::device::phone() {
        phone::columns(ctx, theme, state);
    } else {
        desktop_columns(ctx, theme, state);
    }

    // ── USER PROFILE MODAL ──
    if state.chat_user_modal_open {
        draw_user_modal(ctx, theme, state);
    }
    draw_report_dialog(ctx, theme, state); // the Report dialog, while open (step D)

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

/// Chat's three columns on a desktop: the rooms, the people and the open room between them, and
/// the lock buttons on the open room's top corners. A phone draws `phone::columns` instead.
fn desktop_columns(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
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

    // The lock buttons are part of the page (41+, gui/layers.rs): on its panels' edges, under
    // every floating thing (they were egui's Foreground, over an open dialog's shade).
    crate::gui::layers::area(ctx, egui::Id::new("chat_left_lock_overlay"), crate::gui::layers::Layer::Page)
        .fixed_pos(egui::pos2(left_panel_right, header_top))
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
    crate::gui::layers::area(ctx, egui::Id::new("chat_right_lock_overlay"), crate::gui::layers::Layer::Page)
        .fixed_pos(egui::pos2(right_panel_left - LOCK_PX, header_top))
        .interactable(true)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            if draw_panel_lock_button(ui, theme, state.chat_right_panel_locked) {
                state.chat_right_panel_locked = !state.chat_right_panel_locked;
                crate::config::AppConfig::from_gui_state(state).save();
            }
        });
}

fn draw_center_panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if protected::draws_hidden_room(ui, theme, state) {
        return; // step G: a public room the protected setup leaves out shows one line instead
    }
    // ── Channel header ──
    // Lock buttons moved OUT of the header in v0.189.x — operator wanted
    // them tucked into the actual panel CORNERS, not next to the channel
    // title where they could be mistaken for a UI label. They now paint
    // as floating Areas anchored to the side-panel boundaries (see the
    // overlays at the bottom of `chat::draw`).
    Frame::NONE
        .fill(theme.bg_sidebar_dark())
        .inner_margin(egui::Margin::symmetric(16, 10))
        .stroke(Stroke::new(1.0_f32, theme.border()))
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
                    if widgets::Button::ghost("Back").tooltip("Return to #global").show(ui, theme) {
                        state.chat_active_channel = humanity_protocol::chat_access::MAIN_CHANNEL.to_string();
                    }
                    let partner_key = &ac["dm:".len()..];
                    let partner_name = state.chat_dms.iter()
                        .find(|d| d.user_key == partner_key)
                        .map(|d| d.user_name.clone())
                        .unwrap_or_else(|| crate::engine::dm::dm_display_name(state, partner_key)); // e.g. blocked: not in the list
                    ui.label(
                        RichText::new(format!("DM: {}", partner_name))
                            .size(theme.font_size_heading)
                            .color(theme.dm_accent())
                            .strong(),
                    );
                    report_dialog::draw_dm_header_report(ui, theme, state, partner_key); // step D
                    blocking::draw_dm_header_block(ui, theme, state, partner_key); // step C
                } else if let Some(gid) = ac.strip_prefix("p2pgroup:") {
                    // P2P group header: back button + name + a Copy-invite
                    // action. Leave / Disband live in the left-rail cog/
                    // right-click menu (kept out of this row so it can't
                    // overflow + clip on a narrow panel — that's why the
                    // operator couldn't reach them before).
                    if widgets::Button::ghost("Back").tooltip("Return to #global").show(ui, theme) {
                        state.chat_active_channel = humanity_protocol::chat_access::MAIN_CHANNEL.to_string();
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
                    // The room's linked carriers that are signed in (chat/commons.rs).
                    let linked = commons_carriers(state, &room);
                    let mut carriers: Vec<String> = Vec::new();
                    if linked.active && state.ws_client.as_ref().map_or(false, |c| c.is_connected()) {
                        carriers.push(server_display_name(&state.server_url));
                    }
                    for &i in &linked.parked {
                        if state.connections[i].identified {
                            carriers.push(server_display_name(&state.connections[i].url));
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
                    // Copy link at the header's end (2026-11-03, chat/channel_link.rs): this
                    // channel's address on this server, to send to someone. Only for a channel the
                    // server lists (not the scratch pad, which is this device's alone).
                    if state.chat_channels.iter().any(|c| c.id == state.chat_active_channel) {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let copy = humanity_core::i18n::t("chat.copy_link");
                            if widgets::Button::ghost(copy).hint("chat.copy_link_hint").show(ui, theme) {
                                let (server, id) = (state.active_server(), state.chat_active_channel.clone());
                                channel_link::copy_channel_link(ui.ctx(), state, &server, &id);
                            }
                        });
                    }
                }
            });
        });

    // A message to this person was refused by their "who can reach me" (step B): say so, with
    // Send request, above the conversation.
    if let Some(peer) = state.chat_active_channel.strip_prefix("dm:").map(str::to_string) {
        key_change::draw_key_change_notice(ui, theme, state, &peer); // their key changed: accept first
        reach::draw_refusal_notice(ui, theme, state, &peer);
        transfer_rows::draw(ui, theme, state, &peer); // files going device to device with them
    }

    // ── Message area ──
    let active_channel = state.chat_active_channel.clone();
    let available = ui.available_rect_before_wrap();
    // The input bar's height as last drawn, and what the text box may add (chat/composer.rs).
    let (input_height, composer_cap) = composer::reserve(ui.ctx(), state, composer::Place::Page, available.height());

    let messages_rect = egui::Rect::from_min_size(
        available.min,
        Vec2::new(available.width(), available.height() - input_height),
    );
    // Where the rows drawn now belong and the list's rectangle, for the chat videos' dock
    // (chat/video_dock.rs: it sits at this list's top right, never over the header or the box).
    let here = video_dock::origin_here(state);
    state.chat_video.note_chat_view(here, messages_rect);

    // A Go to message waiting for its row (the chat videos' dock): the list neither sticks to its
    // bottom nor animates while it waits, or egui's stick to the bottom would undo the jump to the
    // row (it puts the offset back at the end every frame the list was at its end).
    let going_to_row = state.chat_video.go_to.is_some();
    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(messages_rect), |ui| {
        ScrollArea::vertical()
            .id_salt("chat_messages_scroll")
            .stick_to_bottom(!going_to_row)
            .animated(!going_to_row)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                rows::draw(ui, theme, state, &active_channel, None);
            });
    });

    // ── Input bar ──
    let input_rect = egui::Rect::from_min_size(
        egui::pos2(available.min.x, available.max.y - input_height),
        Vec2::new(available.width(), input_height),
    );

    composer::draw(ui, theme, state, composer::Place::Page, input_rect, composer_cap);
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
    let holding = resp.is_pointer_button_down_on();
    let (progress, fired) = crate::gui::widgets::hold_step(ui, id, holding, hold_seconds);

    // A clock-style ring on the left fills as you hold, and the arc cycles RGB
    // FAST (faster than the 3s nav channeling) so the wheel is as visible as
    // possible - the operator found the old full-width fill bar "a bit much" and
    // the ring more pleasant. The whole row is still the hold target.
    if resp.hovered() || holding {
        ui.painter().rect_filled(rect, CornerRadius::same(4), theme.bg_card());
    }
    let ring_r = 10.0;
    let ring_c = egui::pos2(rect.left() + 6.0 + ring_r, rect.center().y);
    // Faint full-circle track.
    ui.painter()
        .circle_stroke(ring_c, ring_r, Stroke::new(3.0_f32, theme.border()));
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
            .add(egui::Shape::line(pts, Stroke::new(3.0_f32, col)));
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
    crate::engine::warnings::ensure_loaded(state); // step F's patterns, read once
    reply::drop_if_elsewhere(state); // a reply belongs to the conversation it was started in
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

    // In the world, the chat panel stands where a page's floats do (layer 40, gui/layers.rs).
    let area = crate::gui::layers::area(ctx, egui::Id::new("ingame_chat_panel"), crate::gui::layers::Layer::PageFloat)
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
                                // channel (first known, else #global) exactly
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
                                        .unwrap_or_else(|| humanity_protocol::chat_access::MAIN_CHANNEL.to_string());
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
                        crate::gui::IngameChatMode::Channels => !dm_open && !group_open && !crate::engine::protected::hides_active_room(state),
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
                                .filter(|c| crate::engine::protected::lists_channel(state, c)) // step G
                                .map(|c| (c.id.clone(), c.name.clone(), c.unread))
                                .collect();
                            protected::draw_hidden_rooms_line(ui, theme, state);
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
                                        "Your identity is locked. Unlock it to send direct messages.",
                                    )
                                    .color(theme.warning())
                                    .size(theme.font_size_small),
                                );
                                let (btn_label, target_mode) = match state.auto_unlock_mode {
                                    humanity_core::auto_unlock::AutoUnlockMode::KeychainPin
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
                                let resp = widgets::slider(
                                    ui,
                                    theme,
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
                                let now_ms = line_limit::now_ms();
                                let msgs: Vec<(&str, String, Option<String>, Option<(String, bool)>)> = state
                                    .chat_messages
                                    .iter()
                                    .filter(|m| m.channel == active)
                                    .map(|m| {
                                        // Step G here too: a non-friend's pictures and files are left out, the preset's line instead.
                                        let line = protected::picture_hidden_line(state);
                                        let content = match protected::withheld(state, m) {
                                            Some(rest) if rest.trim().is_empty() || rest == line => line.to_string(),
                                            Some(rest) => format!("{rest} {line}"),
                                            None => m.content.clone(),
                                        };
                                        (
                                            if m.sender_name.is_empty() { "?" } else { m.sender_name.as_str() },
                                            content,
                                            warnings::compact_line(state, m), // step F, short form
                                            // Mine and not (yet) posted: "Sending..." or "Not sent" (line_limit.rs).
                                            line_limit::status_words(m, now_ms).map(|w| (w, matches!(m.send_state, line_limit::LineSend::NotSent { .. }))),
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
                                for (name, content, warn, status) in msgs.iter().rev().take(40).rev() {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(
                                            RichText::new(format!("{name}:"))
                                                .strong()
                                                .color(theme.text_primary())
                                                .size(theme.font_size_small),
                                        );
                                        ui.label(
                                            RichText::new(content.as_str())
                                                .color(theme.text_secondary())
                                                .size(theme.font_size_small),
                                        );
                                    });
                                    if let Some(line) = warn {
                                        ui.label(RichText::new(line).color(theme.warning()).size(theme.font_size_small));
                                    }
                                    if let Some((words, not_sent)) = status {
                                        let color = if *not_sent { theme.danger() } else { theme.text_muted() };
                                        ui.label(RichText::new(words).italics().color(color).size(theme.font_size_small));
                                    }
                                }
                            });
                    }

                    if show_input {
                        // Input line: auto-focused on open, Enter or Send posts
                        // to the active conversation via the same path the Chat
                        // page uses (send_composed_content routes channel /
                        // E2EE-DM / group / P2P-group identically).
                        ui.horizontal(|ui| {
                            // The characters past the line limit, red on dark red, as in the page's box.
                            let mut over_limit = line_limit::limit_here(state).map(|n| widgets::over_limit::layouter(theme, n, true));
                            let mut edit = egui::TextEdit::singleline(&mut state.chat_input)
                                // Stable id (v0.972): same focus-loss
                                // protection as the main composer.
                                .id(egui::Id::new("chat_overlay_input"))
                                .desired_width(390.0)
                                .hint_text("Message... (Enter to send, Esc to close)");
                            if let Some(l) = over_limit.as_mut() {
                                edit = edit.layouter(l);
                            }
                            let resp = ui.add(edit);
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
        state.chat_reply_to = None; // a reply belongs to the conversation it was started in
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
        state.go(crate::gui::GuiPage::Chat, crate::gui::How::Beside);
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
pub(crate) fn send_composed_content(state: &mut GuiState, content: &str) -> bool {
    // A line starting with `/claim` (zero-width characters ignored, any case, a space, a colon or
    // the code right after, or nothing) goes to the server alone or shows the claim help, before any
    // routing: never to the people of an open DM or group (the owner review's finding 6 and the
    // second review's W5, engine/owner.rs `typed_claim`).
    if crate::engine::owner::typed_claim(state, content) {
        return true;
    }
    let channel = state.chat_active_channel.clone();
    // Step F: the recovery-phrase guard, before anything leaves (the local scratchpad sends nothing);
    // step G: a typed `/redeem` makes a friend, so the protected setup asks its PIN first.
    if channel != "scratchpad" && (crate::engine::warnings::guard_stops(state, &[content]) || !crate::engine::protected::allows_typed(state, content)) {
        return false; // nothing leaves; the draft stays so the phrase can be removed
    }
    // A typed /friend-code or /redeem: the one answer it asks for is taken (engine/friend_code.rs).
    // Only where the line reaches the server as a command: in a DM or a group it is sealed for the
    // people there, so the server never sees it and no answer is owed (the eighth review).
    if channel != "scratchpad" && !channel.starts_with("dm:") && !channel.starts_with("p2pgroup:") {
        crate::engine::friend_code::note_sent(state, content);
    }
    // Single timestamp for both the WS send and the local echo so
    // reaction-targeting (which keys on sender + ts) matches.
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let mut send_aborted = false;
    // A room line put on the active socket: its copy reads "Sending..." until the server's comes
    // back, or "Not sent" if the server refuses it (chat/line_limit.rs). A DM, a group's line and
    // the scratch pad are not followed (the server neither echoes nor refuses them that way).
    let mut room_line_sent = false;

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
    let (active_carries_room, carrier_idx) = match commons_room.as_deref() {
        Some(room) => commons_route(state, room),
        None => (true, None), // plain channels always use the active socket
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
        let partner_key = channel["dm:".len()..].to_string();
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
                let human = key_change::why_not_sent(state, reason, &partner_key); // in plain words
                // Surface the reason INSIDE the conversation so it can't
                // be missed, and keep the draft in the composer. The app's own voice, built only
                // from its words and typed arguments (state_types/app_voice.rs, the eighth review).
                let server = norm_server_url(&state.server_url);
                state.chat_messages.push(crate::gui::app_row(human, channel.clone(), server, ts));
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
                            humanity_core::debug::push_debug(format!("WS >>> {}", self_put));
                            client.send(&self_put);
                            Some(recipient_put)
                        }
                        None => None,
                    }
                } else if channel.starts_with("group:") {
                    let group_id = &channel["group:".len()..];
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
                    // Only a reply started in this same conversation, and never one to a private
                    // message: its words would land in a public room (chat/reply.rs).
                    if let Some(r) = reply::wire_reply(state, &channel) {
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
                    humanity_core::debug::push_debug(format!("WS >>> {}", json_str));
                    client.send(&json_str);
                    room_line_sent = line_limit::measured(&channel);
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
        if let Some(r) = reply::wire_reply(state, &channel) {
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
        let local_reply = reply::made_in(state, &channel);
        let now = chrono_now_str();
        let conn = &mut state.connections[ci];
        if let Some(ws) = conn.ws.as_ref() {
            humanity_core::debug::push_debug(format!("WS(bg) >>> {}", json_str));
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
            send_state: line_limit::LineSend::Sending, // until its copy comes back (chat/line_limit.rs)
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
    let local_reply_to = reply::made_in(state, &channel); // shown on our own copy only
    if is_scratchpad {
        // Kept on this device, encrypted, as the web does (10m R10): typed notes and a file's
        // marker (attach_send.rs drains into here), whose key exists nowhere else.
        crate::engine::dm::keep_scratch_note(state, ts, content, local_reply_to.as_ref());
    }
    // Keep the DM sidebar preview current for our own sends (v0.715).
    if let Some(pk) = channel.strip_prefix("dm:") {
        if let Some(d) = state.chat_dms.iter_mut().find(|d| d.user_key == pk) {
            // A file's marker carries its key; the preview says "Photo" or its name instead.
            d.last_message = format!("You: {}", crate::engine::dm::dm_preview_text(content));
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
        send_state: if room_line_sent { line_limit::LineSend::Sending } else { line_limit::LineSend::Posted },
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
    "png", "jpg", "jpeg", "gif", "webp", "heic", "heif", "avif", "pdf", "txt", "md", "json",
    "zip", "tar.gz", "mp3", "mp4", "webm", "ogg", "wav",
    "blend", "stl", "obj", "gltf", "glb",
];

/// 3D/model formats auto-publish to the public Shared Files library
/// (upload with ?share=1), exactly like the web client.
const SHARE_EXTS: &[&str] = &["blend", "stl", "obj", "gltf", "glb"];

/// The limit on a file, private or public, until the server says the person's own
/// (`GuiState::upload_limits`, from its peer list, for that server and identity; the relay's
/// default private limit is this same 6 MB). chat/attach_send.rs `limit_for`.
pub(crate) const ATTACH_MAX_BYTES: u64 = 6 * 1024 * 1024;

/// Best-effort MIME from the filename extension (server re-checks anyway).
pub(crate) fn mime_for_filename(name: &str) -> &'static str {
    let lower = name.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        // iPhone photos: the relay cleans their location and keeps them as plain files.
        "heic" => "image/heic",
        "heif" => "image/heif",
        "avif" => "image/avif",
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

/// The placeholder drawn in place of a picture from another website, under a
/// message (2026-10-09). Nothing is fetched until it is clicked, and it says
/// why in plain words: loading the picture shows that website the reader's
/// network address. The web client shows every picture this way; native loads
/// its own server's pictures by itself and asks only for other websites'.
/// Returns true on the click that allows it. The box itself is
/// `attach_view::draw_ask_box`, shared with a private file's picture.
fn draw_click_to_load(ui: &mut egui::Ui, theme: &Theme, row_bg: Color32, indent: f32, host: &str) -> bool {
    let first = format!("Picture from {host}. Click to load it.");
    attach_view::draw_ask_box(ui, theme, row_bg, indent, &first, &format!("Loading this shows {host} your network address."))
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

/// The COMMONS sidebar section: federated rooms carried by 2+ of my
/// servers, each rendered ONCE. Clicking opens the room exactly like a
/// normal channel (the center view then merges every carrier's copy).
fn draw_commons_section(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // Step G: with the protected setup on, only read-only rooms are listed.
    let rooms: Vec<CommonsRoom> = commons_rooms(state).into_iter().filter(|r| crate::engine::protected::lists_commons_room(state, &r.name)).collect();
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
                    // A line of the body text tall, as every sidebar row (chat/channel_row.rs, BUG-243).
                    Vec2::new(row_w, channel_row::text_row_height(ui, theme)),
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
                        Vec2::new(theme.row_accent_width, row_rect.height()),
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
            // Laid out as paint_timestamp_pill lays it out: the emoji's picture, when there is one,
            // takes its placeholder's width (widgets/emoji/inline.rs).
            let font = egui::FontId::proportional(theme.small_size);
            let label_w = crate::gui::widgets::emoji::layout_singleline(ctx, label, font, theme.text_primary()).0.size().x;
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
        Stroke::new(1.0_f32, theme.border()),
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
            // The emoji as its picture when there is one (widgets/emoji/inline.rs, rung 3).
            let label_color = if i_reacted { theme.accent() } else { theme.text_primary() };
            let font = egui::FontId::proportional(theme.small_size);
            let (label_galley, label_pictures) = crate::gui::widgets::emoji::layout_singleline(ui.ctx(), label.clone(), font, label_color);
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
            painter.rect_filled(badge_rect, CornerRadius::same(7), badge_bg);
            let label_pos = egui::pos2(badge_rect.left() + 2.0, cy - label_galley.size().y / 2.0);
            painter.galley(label_pos, label_galley.clone(), label_color);
            crate::gui::widgets::emoji::paint(painter, label_pos, &label_galley, &label_pictures);
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

/// Where to file a server-level system / private-server notice (relay messages,
/// deploy-bot announcements, command responses). These were tagged with the
/// *active* channel — but when the user is viewing a private P2P context (a
/// `p2pgroup:` group or a `dm:` conversation), that leaks the notice INTO the
/// private view, where it then vanishes on the next group reload (which rebuilds
/// strictly from the group's signed message log). So when the active channel is
/// private, file the notice under #global (`MAIN_CHANNEL`, a server channel) instead — it's
/// preserved without polluting the private conversation. (Bug: a deploy-bot
/// #announcements notice appeared inside an open P2P group, then disappeared.)
/// A DM (`dm:`) or a P2P group (`p2pgroup:`): encrypted end to end, so nothing that sends a
/// message's text or who reacted to it to the server (pin, edit, reaction, delete) is offered or
/// sent there (BUG-178, 2026-10-10). The relay refuses them too (`relay::handlers::utils::is_room`).
/// And the scratchpad, which its label calls local-only: an edit or a pin there would carry its
/// words to the server, though the server keeps none of it (the web chat's scratch pad rows are
/// private rows the same way, app.js `SCRATCH_PAD_ROW`).
pub(crate) fn is_private_channel(channel: &str) -> bool {
    channel.starts_with("dm:") || channel.starts_with("p2pgroup:") || channel == "scratchpad"
}

/// The composer was edited (v0.282.0): send `typing`, at most once every 3 seconds (the relay's
/// own silent-drop limit, so nothing is sent only to be dropped), not when the box was cleared
/// (that is not typing), not while disconnected, and never while the scratch pad is open (10m
/// R10: it sends nothing, so nobody should hear that the person is writing there).
pub(crate) fn send_typing(state: &mut GuiState, now: std::time::Instant) {
    if state.chat_input.is_empty() || state.chat_active_channel == "scratchpad" {
        return;
    }
    if state.chat_typing_last_sent.is_some_and(|t| now.duration_since(t).as_secs() < 3) {
        return;
    }
    let Some(client) = state.ws_client.as_ref().filter(|c| c.is_connected()) else { return };
    let m = serde_json::json!({
        "type": "typing",
        "from": state.profile_public_key,
        "from_name": state.user_name,
    });
    client.send(&m.to_string());
    state.chat_typing_last_sent = Some(now);
}

pub fn notice_channel(active_channel: &str) -> String {
    if active_channel.starts_with("p2pgroup:") || active_channel.starts_with("dm:") {
        humanity_protocol::chat_access::MAIN_CHANNEL.to_string()
    } else if let Some(room) = commons_room_of(active_channel) {
        // A Commons VIEW id is not a storable channel; file the notice
        // under the underlying room so it renders in the merged view
        // (when the active server carries it) and is never orphaned.
        room.to_string()
    } else {
        active_channel.to_string()
    }
}

/// The server's channel list just arrived: when the room open on this server is not in it (an
/// admin deleted it, or the server renamed it, as #general became #global on 2026-11-02), land in
/// #global, else in the first room the server lists, and fetch its history. A DM, a group, a
/// Commons view and the scratch pad are not this server's rooms and are left as they are. This is
/// what any deleted room does, not a mapping from an old name: no name is carried over.
pub(crate) fn land_if_room_gone(state: &mut GuiState) {
    let active = state.chat_active_channel.as_str();
    let not_a_room = is_private_channel(active) || active.starts_with("group:") || commons_room_of(active).is_some();
    if not_a_room || state.chat_channels.is_empty() || state.chat_channels.iter().any(|c| c.id == active) {
        return;
    }
    let main = humanity_protocol::chat_access::MAIN_CHANNEL;
    let to = if state.chat_channels.iter().any(|c| c.id == main) {
        main.to_string()
    } else {
        state.chat_channels[0].id.clone()
    };
    state.chat_active_channel = to;
    state.chat_reply_to = None;
    state.chat_messages.clear();
    state.history_fetched = false;
}

/// Try to read an image from the OS clipboard and encode it as PNG bytes.
/// Returns `None` when the clipboard has no image (e.g. just text), or when
/// the clipboard / encoder errors. The native arboard crate handles
/// Windows / macOS / Linux clipboard access. v0.232.
///
/// Not on a phone (Android rung A1): arboard has no Android code (Cargo.toml leaves it out there),
/// and a phone has no Ctrl+V to ask for this; pictures go in through Attach. Words still paste
/// through egui.
#[cfg(target_os = "android")]
fn try_grab_clipboard_image_as_png() -> Option<Vec<u8>> {
    None
}

#[cfg(not(target_os = "android"))]
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
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.random()).collect();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Derive a consistent color from a username (for avatar circles).
/// Parse a role's "#RRGGBB" badge color into a Color32. Falls back to
/// the theme's primary text color if the string isn't a valid 6-digit
/// hex (so a malformed custom-role color can't make the label invisible).
/// v0.241 (roles Phase R2).
fn parse_role_color(hex: &str, theme: &Theme) -> Color32 {
    let h = hex.trim().trim_start_matches('#');
    // Six hex digits, read whole: the colour comes from the server, and cutting other text two
    // bytes at a time could fall inside a character and panic (BUG-340).
    if h.len() == 6 && h.bytes().all(|c| c.is_ascii_hexdigit()) {
        let [_, r, g, b] = u32::from_str_radix(h, 16).unwrap_or(0).to_be_bytes();
        return Color32::from_rgb(r, g, b); // theme-exempt: data-driven role color from server
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

/// Send a slash command as a chat message (server handles moderation via slash commands).
/// `pub(crate)` for the protected setup, which sends a `/redeem` once its PIN is entered.
pub(crate) fn send_slash_command(state: &mut GuiState, command: &str) {
    if state.ws_client.as_ref().is_some_and(|c| c.is_connected()) {
        crate::engine::friend_code::note_sent(state, command); // the one answer it asks for
    }
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
///   - `"missing_peer_key"`  — no key for them from a key card checked here (engine/key_cards.rs)
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

#[cfg(test)]
mod private_channel_tests {
    /// BUG-178: a DM and a P2P group are private, so a row there offers no pin, edit, reaction
    /// or delete, and none is sent; a room of the server is not private. Seen red 2026-10-10 with
    /// `is_private_channel` answering false for groups: "a P2P group is private"; and before the
    /// scratchpad counted: "the scratchpad, local only, is private".
    #[test]
    fn dms_and_p2p_groups_are_private_and_rooms_are_not() {
        assert!(super::is_private_channel("dm:abc"), "a DM is private");
        assert!(super::is_private_channel("p2pgroup:xyz"), "a P2P group is private");
        assert!(super::is_private_channel("scratchpad"), "the scratchpad, local only, is private");
        for room in ["global", "announcements", "local", "commons:garden", ""] {
            assert!(!super::is_private_channel(room), "{room:?} is not private");
        }
    }

    /// A ROOM THAT IS GONE LANDS IN #GLOBAL (2026-11-02, docs/design/chat-channels.md): a
    /// desktop app open in #general when its server renamed it to #global, or in a room an admin
    /// deleted, is in #global once the channel list arrives, else in the first room listed, with
    /// its history to fetch; a room that is there, a DM, a group, a Commons view and the scratch pad
    /// stay where they are; the channel list's arrival calls it (frame_ws_poll.rs). Seen red
    /// 2026-11-02 with the function returning at once: "a room that is gone lands in #global"
    /// (left: "general", right: "global").
    #[test]
    fn a_room_that_is_gone_lands_in_global() {
        use crate::gui::ChatChannel;
        let room = |id: &str| ChatChannel { id: id.into(), name: id.into(), ..Default::default() };
        let mut state = crate::gui::GuiState::default();
        state.chat_channels = vec![room("announcements"), room("local"), room("global"), room("feedback")];
        state.chat_active_channel = "general".into();
        state.history_fetched = true;
        super::land_if_room_gone(&mut state);
        assert_eq!(state.chat_active_channel, "global", "a room that is gone lands in #global");
        assert!(!state.history_fetched, "and fetches its history");
        state.chat_channels.retain(|c| c.id != "global");
        state.chat_active_channel = "deleted".into();
        super::land_if_room_gone(&mut state);
        assert_eq!(state.chat_active_channel, "announcements", "without #global, the first room listed");
        for stays in ["local", "dm:abc", "p2pgroup:xyz", "commons:garden", "scratchpad"] {
            state.chat_active_channel = stays.into();
            super::land_if_room_gone(&mut state);
            assert_eq!(state.chat_active_channel, stays, "{stays} stays");
        }
    }
}

/// 10m R10: the scratch pad stays on this device, and keeps what is put in it.
#[cfg(test)]
mod scratchpad_tests {
    use super::*;

    /// One headless frame (no GPU) of the left panel, a click on `text` in it.
    fn click(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, text: &str) {
        let frame = |state: &mut GuiState, events: Vec<egui::Event>| {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(320.0, 900.0))), events, ..Default::default() };
            ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| draw_left_panel(ui, theme, state));
            })
        };
        let out = frame(state, Vec::new());
        let pos = crate::gui::screen_surface::find_text_in_shapes(&out.shapes, text).unwrap_or_else(|| panic!("{text} is not drawn")).rect.center();
        let m = egui::Modifiers::default();
        frame(state, vec![egui::Event::PointerMoved(pos)]);
        frame(state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
        frame(state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
    }

    /// THE DESKTOP KEEPS SCRATCH PAD NOTES (10m R10, as the web does): a typed note (with its reply
    /// quote) and a file's marker (the only copy of its key) are kept in the encrypted DM store, so
    /// opening the scratch pad again, from its row in the left rail, and restarting both bring them
    /// back; and no typing indicator goes out while it is open, as one does in a channel.
    /// Seen red 2026-10-10 two ways: with the `keep_scratch_note` call taken out of
    /// `send_composed_content`, "kept across opening it again" failed (0 notes, the old behaviour);
    /// and with `send_typing` not asking about the scratch pad, "no typing indicator from the
    /// scratch pad" failed.
    #[test]
    fn scratchpad_notes_are_kept_and_it_sends_no_typing() {
        crate::config::keep_saves_off_disk();
        let mut state = GuiState::default();
        state.onboarding_complete = true;
        state.user_name = "Ada".to_string();
        state.private_key_bytes = Some(vec![171u8; 32]);
        state.apply_pq_identity();
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        state.server_url = format!("https://scratch-{nanos}.example");
        state.block_list = Some(crate::net::block_list::BlockList::in_temp(&[171u8; 32], &state.profile_public_key, "scratch"));
        let (client, sent) = crate::net::ws_client::WsClient::recording();
        state.ws_client = Some(client);
        let (ctx, theme) = (egui::Context::default(), crate::gui::theme::load_theme());
        crate::gui::fonts::install_font_fallbacks(&ctx);
        theme.apply_to_egui(&ctx);

        state.chat_active_channel = "global".into();
        click(&ctx, &theme, &mut state, "# scratchpad");
        assert_eq!(state.chat_active_channel, "scratchpad");
        state.chat_input = "x".into();
        send_typing(&mut state, std::time::Instant::now());
        assert!(sent.try_iter().next().is_none(), "no typing indicator from the scratch pad");

        state.chat_reply_to = Some(crate::gui::ReplyContext { sender_name: "Ada".into(), preview: "seeds to buy".into(), timestamp_ms: 5, conversation: "scratchpad".into(), private: true, ..Default::default() });
        assert!(send_composed_content(&mut state, "beans, squash"));
        let marker = crate::net::dm_pq::build_file_marker(&crate::net::dm_pq::DmAttachment { url: "/uploads/a.enc".into(), k: "k".into(), n: "n".into(), name: "map.jpg".into(), mime: "image/jpeg".into(), size: 3 });
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Ok(marker.clone())).unwrap();
        state.clipboard_upload = Some(("scratchpad".to_string(), rx));
        attach_send::drain(&ctx, &mut state);
        assert!(sent.try_iter().next().is_none(), "the scratch pad sends nothing");

        let shown = |state: &GuiState| state.chat_messages.iter().filter(|m| m.channel == "scratchpad").map(|m| (m.content.clone(), m.reply_to.as_ref().map(|r| r.preview.clone()))).collect::<Vec<_>>();
        let want = vec![("beans, squash".to_string(), Some("seeds to buy".to_string())), (marker.clone(), None)];
        state.chat_active_channel = "global".into();
        state.chat_messages.clear();
        click(&ctx, &theme, &mut state, "# scratchpad");
        assert_eq!(shown(&state), want, "kept across opening it again");
        state.dm_store = None; // a restart: the store is read from disk again
        state.chat_active_channel = "global".into();
        click(&ctx, &theme, &mut state, "# scratchpad");
        assert_eq!(shown(&state), want, "and across a restart");

        state.chat_active_channel = "global".into();
        send_typing(&mut state, std::time::Instant::now());
        assert_eq!(sent.try_iter().filter(|f| f.contains("\"typing\"")).count(), 1, "a channel still sends one");
        state.dm_store.as_ref().unwrap().remove_file_for_test();
        state.block_list.as_ref().unwrap().remove_file_for_test();
    }
}

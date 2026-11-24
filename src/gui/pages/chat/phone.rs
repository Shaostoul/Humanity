//! Chat on a phone: one column at a time (Android rung A1, 2026-11-14; docs/design/platforms.md).
//!
//! The desktop's Chat is three columns side by side: the rooms (at least 150 points), the open room,
//! and the people (at least 150 points). On the operator's phone the window is about 412 points
//! wide, which would leave the messages about a hundred. So a phone shows ONE column, the way phone
//! chat apps do: a conversation first (the last room open on this server, else #global); a Rooms
//! button above it opens the rooms list, whose Close button (or a tap on any room, the open one
//! too) comes back to the conversation. The people column is not shown on a phone yet (the
//! room's own header still names it); a later step gives it a button of its own.
//!
//! Which column shows lives in egui's memory for the run, not in the saved settings: the app opens
//! on the conversation. (Operator, 2026-11-23: "when I log in on my phone and go to chat have it load
//! the last channel or default to global. Right now it loads the server list".)

use egui::Frame;
use humanity_core::i18n::t;

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// The egui memory key for "the rooms list is up" (the tests set it).
const SHOWS_ROOMS: &str = "chat_phone_shows_rooms";
/// Set for the frame a room row was tapped, selected already or not.
const ROOM_TAPPED: &str = "chat_phone_room_tapped";

/// A room row (or a conversation row) was tapped: the list closes on the conversation even when
/// that room was already the open one (2026-11-23: the list moved on only when the selection
/// CHANGED, so tapping #global while #global was selected left the list up over the chat).
pub(super) fn room_tapped(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(ROOM_TAPPED), true));
}

/// Draw Chat's columns as a phone shows them (the module docs).
pub(super) fn columns(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let id = egui::Id::new(SHOWS_ROOMS);
    // A phone opens on a conversation: the room this person had open on this server (kept per
    // server in `chat_active_channel`), else #global, which is that field's default.
    let mut rooms = ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    if rooms {
        let open_before = state.chat_active_channel.clone();
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(ROOM_TAPPED), false));
        egui::TopBottomPanel::top("chat_phone_close")
            .frame(Frame::NONE.fill(theme.bg_sidebar_dark()).inner_margin(theme.spacing_xs))
            .show(ctx, |ui| {
                if widgets::Button::secondary(t("chat.phone.close")).hint("chat.phone.close_hint").show(ui, theme) {
                    rooms = false;
                }
            });
        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(theme.bg_sidebar_dark()).inner_margin(0.0))
            .show(ctx, |ui| {
                super::draw_left_panel(ui, theme, state);
            });
        // Picking a room opens it, a different one or the one already open.
        let tapped = ctx.data(|d| d.get_temp::<bool>(egui::Id::new(ROOM_TAPPED))).unwrap_or(false);
        if tapped || state.chat_active_channel != open_before {
            rooms = false;
        }
    } else {
        egui::TopBottomPanel::top("chat_phone_back")
            .frame(Frame::NONE.fill(theme.bg_sidebar_dark()).inner_margin(theme.spacing_xs))
            .show(ctx, |ui| {
                if widgets::Button::secondary(t("chat.phone.rooms")).hint("chat.phone.rooms_hint").show(ui, theme) {
                    rooms = true;
                }
            });
        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(theme.bg_panel()).inner_margin(0.0))
            .show(ctx, |ui| {
                super::draw_center_panel(ui, theme, state);
            });
    }
    ctx.data_mut(|d| d.insert_temp(id, rooms));
}

//! Chat on a phone: one column at a time (Android rung A1, 2026-11-14; docs/design/platforms.md).
//!
//! The desktop's Chat is three columns side by side: the rooms (at least 150 points), the open room,
//! and the people (at least 150 points). On the operator's phone the window is about 412 points
//! wide, which would leave the messages about a hundred. So a phone shows ONE column, the way phone
//! chat apps do: the rooms list first; picking a room opens it across the whole width, and a Rooms
//! button above it goes back to the list. The people column is not shown on a phone yet (the
//! room's own header still names it); a later step gives it a button of its own.
//!
//! Which column shows lives in egui's memory for the run, not in the saved settings: the app opens
//! on the rooms list, as a phone chat app does.

use egui::Frame;
use humanity_core::i18n::t;

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// Draw Chat's columns as a phone shows them (the module docs).
pub(super) fn columns(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let id = egui::Id::new("chat_phone_shows_rooms");
    let mut rooms = ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(true);
    if rooms {
        let open_before = state.chat_active_channel.clone();
        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(theme.bg_sidebar_dark()).inner_margin(0.0))
            .show(ctx, |ui| {
                super::draw_left_panel(ui, theme, state);
            });
        // Picking a room opens it.
        if state.chat_active_channel != open_before {
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

//! Mute server and Mute room (2026-10-30, docs/design/notifications.md, section 5): the items in
//! the active server's cog menu, a saved server's right-click menu and a room's right-click menu,
//! and the muted mark on a server's row.
//!
//! A mute stops notices (the sound and the operating system's notice) from that server's rooms or
//! that room, for 1 hour, 8 hours or until unmuted (the lengths are data:
//! `data/notifications/notifications.json` `mute_for`). It never hides a message or an unread dot,
//! and a server's mute leaves direct messages alone (a DM is a person, not the server). Kept on
//! this device with the other notification choices (`config.json` `notifications.mutes`), and
//! listed with Unmute in Settings > Notifications.

use egui::RichText;

use crate::gui::i18n::{t, tf};
use crate::gui::pages::settings::notifications::{mute_choices, until_words};
use crate::gui::theme::Theme;
use crate::gui::widgets::Hint;
use crate::gui::GuiState;
use crate::notifications::now_secs;

use super::norm_server_url;

/// Until when the server at `url` is muted, if it is now.
pub(crate) fn server_mute(state: &GuiState, url: &str) -> Option<i64> {
    state.settings.notifications.mutes.server_muted(&norm_server_url(url), now_secs())
}

/// The server's items: "Muted until 14:00" and Unmute server, or Mute server and its lengths.
pub(crate) fn server_items(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, url: &str) {
    let key = norm_server_url(url);
    let now = now_secs();
    match state.settings.notifications.mutes.server_muted(&key, now) {
        Some(until) => {
            muted_line(ui, theme, until);
            if ui.button(t("notifications.unmute_server")).hint("notifications.unmute_hint").clicked() {
                state.settings.notifications.mutes.unmute_server(&key);
                state.settings_dirty = true;
                ui.close_menu();
            }
        }
        None => {
            heading(ui, theme, t("notifications.mute_server"), "notifications.mute_server_hint");
            if let Some(seconds) = mute_choices(ui) {
                state.settings.notifications.mutes.mute_server(&key, now, seconds);
                state.settings_dirty = true;
            }
        }
    }
}

/// A room's items, as the server's.
pub(crate) fn room_items(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, url: &str, room: &str) {
    let key = norm_server_url(url);
    let now = now_secs();
    match state.settings.notifications.mutes.room_muted(&key, room, now) {
        Some(until) => {
            muted_line(ui, theme, until);
            if ui.button(t("notifications.unmute_room")).hint("notifications.unmute_hint").clicked() {
                state.settings.notifications.mutes.unmute_room(&key, room);
                state.settings_dirty = true;
                ui.close_menu();
            }
        }
        None => {
            heading(ui, theme, t("notifications.mute_room"), "notifications.mute_room_hint");
            if let Some(seconds) = mute_choices(ui) {
                state.settings.notifications.mutes.mute_room(&key, room, now, seconds);
                state.settings_dirty = true;
            }
        }
    }
}

/// The small heading above the lengths, with what muting does on hover.
fn heading(ui: &mut egui::Ui, theme: &Theme, words: &str, hint: &'static str) {
    ui.label(RichText::new(words).size(theme.font_size_small).color(theme.text_secondary()).strong()).hint(hint);
}

fn muted_line(ui: &mut egui::Ui, theme: &Theme, until: i64) {
    ui.label(RichText::new(tf("notifications.muted_notice", &[("until", &until_words(until))])).size(theme.font_size_small).color(theme.text_secondary()));
}

/// The muted mark on a server row: a bell with a slash centred at `center`, in place of the row's
/// link dot and in its `color` (so whether the server is up still shows), saying until when on
/// hover.
pub(crate) fn paint_muted_mark(ui: &mut egui::Ui, center: egui::Pos2, color: egui::Color32, id: egui::Id, until: i64) {
    let r = egui::Rect::from_center_size(center, egui::vec2(12.0, 12.0));
    crate::gui::widgets::icons::paint_bell_off(ui.painter(), r, color);
    let words = tf("notifications.muted_mark_hint", &[("until", &until_words(until))]);
    ui.interact(r, id, egui::Sense::hover()).on_hover_text(words);
}

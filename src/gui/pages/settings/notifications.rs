//! Settings > Notifications (2026-10-30, docs/design/notifications.md, section 5): the whole set,
//! mirroring the website's Settings > Notifications.
//!
//! Three cards:
//! - **On this computer** (kept in `config.json` `notifications`): desktop notices, what a notice
//!   shows (who it is from by default; the message only by choice), the sound and which one with
//!   Play, Show a test notification, and (rung 2, part 2) the tray icon and whether closing the
//!   window keeps HumanityOS running in it (engine/tray.rs, engine/tray_window.rs).
//! - **What notifies you** (the person's own switches, kept on their server, `notification_prefs`):
//!   direct messages, mentions, task updates, and quiet hours by the hour. A change goes to EVERY
//!   server this device is signed in to ([`send_synced`]), so each one's Web Push agrees.
//! - **Muted**: Mute everything, and every server and room muted, each with Unmute; and the
//!   unfinished-software warning bar, muted until when with Show it again, or Mute
//!   (gui/bottom_strip/mute.rs `settings_row`).
//!
//! Every word is a key in `data/i18n/en.json`; every control carries its plain-language hint.

use egui::{Color32, RichText};

use humanity_core::i18n::{t, t_dyn, tf};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, with_hint, Hint};
use crate::gui::GuiState;
use crate::notifications::{self as n, Show};

/// The words of a "show" choice: its label and its hint.
fn show_words(s: Show) -> (&'static str, &'static str) {
    match s {
        Show::Nothing => ("notifications.show_nothing", "notifications.show_nothing_hint"),
        Show::Name => ("notifications.show_name", "notifications.show_name_hint"),
        Show::NameAndText => ("notifications.show_name_and_text", "notifications.show_name_and_text_hint"),
    }
}

/// The person's synced switches as the relay takes them: all five, quiet hours off as null (which
/// the relay reads as "clear them"). `None` until the server has said what it holds
/// (`notif_prefs_loaded`): before that this device has only the defaults, and sending them would
/// undo what the person chose on another device (2026-10-30). Every write goes through here.
pub(crate) fn synced_frame(state: &GuiState) -> Option<String> {
    if !state.notif_prefs_loaded {
        return None;
    }
    Some(serde_json::json!({
        "type": "update_notification_prefs",
        "dm": state.notif_dm_enabled,
        "mentions": state.notif_mentions_enabled,
        "tasks": state.notif_tasks_enabled,
        "dnd_start": state.notif_dnd_start,
        "dnd_end": state.notif_dnd_end,
        "quiet_friends": state.notif_quiet_friends,
        "quiet_mentions": state.notif_quiet_mentions,
    })
    .to_string())
}

/// Send the person's synced switches to every server this device is signed in to: the active one
/// and each saved server's background link (the switches are the person's, not one server's).
pub(crate) fn send_synced(state: &GuiState) {
    let Some(frame) = synced_frame(state) else { return };
    if let Some(c) = state.ws_client.as_ref().filter(|c| c.is_connected()) {
        c.send(&frame);
    }
    for conn in state.connections.iter().filter(|c| c.identified) {
        if let Some(ws) = conn.ws.as_ref() {
            ws.send(&frame);
        }
    }
}

/// "until 14:00" or "until you unmute it".
pub(crate) fn until_words(until: i64) -> String {
    if until == 0 {
        t("notifications.muted_until_unmuted").to_string()
    } else {
        let time = crate::gui::pages::chat::format_timestamp((until.max(0) as u64) * 1000);
        tf("notifications.muted_until", &[("time", &time)])
    }
}

/// A server's name for the muted list: its saved name, else its address.
fn server_name(state: &GuiState, url: &str) -> String {
    let norm = crate::gui::pages::chat::norm_server_url;
    state
        .chat_servers
        .iter()
        .find(|s| norm(&s.url) == url)
        .map(|s| s.name.clone())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| url.to_string())
}

/// The mute-length choices as menu items; `Some(seconds)` for the one clicked (0: until unmuted).
pub(crate) fn mute_choices(ui: &mut egui::Ui) -> Option<u64> {
    let mut chosen = None;
    for m in &n::data::get().mute_for {
        if ui.button(t_dyn(&m.label)).hint_dyn(&m.hint).clicked() {
            chosen = Some(m.seconds);
            ui.close_menu();
        }
    }
    chosen
}

/// Settings > Notifications.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: Color32) {
    let hint = state.settings.hint_display;
    let now = n::now_secs();
    if state.settings.notifications.mutes.prune(now) {
        state.settings_dirty = true;
    }

    // ── On this computer ──
    widgets::subsection_header(ui, theme, accent, t("notifications.device_heading"), "");
    widgets::card(ui, theme, |ui| {
        widgets::setting_hint(ui, theme, hint, t("notifications.device_note"));
        let s = &mut state.settings.notifications;
        let mut dirty = false;
        dirty |= with_hint(ui, "notifications.desktop_hint", |ui| widgets::toggle(ui, theme, t("notifications.desktop"), &mut s.desktop));
        widgets::setting_hint(ui, theme, hint, t("notifications.desktop_hint"));

        ui.label(RichText::new(t("notifications.show")).color(theme.text_primary()));
        let before = s.show;
        for choice in Show::ALL {
            let (label, choice_hint) = show_words(choice);
            ui.radio_value(&mut s.show, choice, RichText::new(t(label)).color(theme.text_primary())).hint(choice_hint);
        }
        dirty |= s.show != before;
        widgets::setting_hint(ui, theme, hint, t("notifications.show_hint"));

        dirty |= with_hint(ui, "notifications.sound_hint", |ui| widgets::toggle(ui, theme, t("notifications.sound"), &mut s.sound));
        widgets::setting_hint(ui, theme, hint, t("notifications.sound_hint"));

        // Sounds (rung 3): one row per kind that notifies, its sound or None, with Play.
        let file = n::data::get();
        let mut preview = None;
        ui.label(RichText::new(t("notifications.sounds_heading")).color(theme.text_primary()));
        widgets::setting_hint(ui, theme, hint, t("notifications.sounds_note"));
        ui.add_enabled_ui(s.sound, |ui| {
            for kind in file.kinds.iter().filter(|k| k.built) {
                let current = s.sound_of(file, &kind.id).map(str::to_string);
                let shown = current.as_deref().and_then(|id| file.sounds.iter().find(|x| x.id == id)).map_or(t("notifications.sound_none"), |x| t_dyn(&x.label));
                let mut picked: Option<String> = None;
                widgets::form_row(ui, theme, t_dyn(&kind.label), |ui| {
                    egui::ComboBox::from_id_salt(("notify_sound_combo", kind.id.as_str()))
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            for snd in &file.sounds {
                                if ui.selectable_label(current.as_deref() == Some(snd.id.as_str()), t_dyn(&snd.label)).hint("notifications.sound_kind_hint").clicked() {
                                    picked = Some(snd.id.clone());
                                }
                            }
                            if ui.selectable_label(current.is_none(), t("notifications.sound_none")).hint("notifications.sound_kind_hint").clicked() {
                                picked = Some(n::settings::NO_SOUND.to_string());
                            }
                        })
                        .response
                        .hint("notifications.sound_kind_hint");
                    ui.add_enabled_ui(current.is_some(), |ui| {
                        if widgets::Button::secondary(t("notifications.sound_play")).hint("notifications.sound_play_hint").show(ui, theme) {
                            preview = current.clone();
                        }
                    });
                });
                if let Some(id) = picked {
                    s.set_sound(file, &kind.id, &id);
                    dirty = true;
                }
            }
        });
        if preview.is_some() {
            state.notify.preview_sound = preview;
        }

        let shown_at = egui::Id::new("notify_test_shown_at");
        if widgets::Button::secondary(t("notifications.test")).hint("notifications.test_hint").show(ui, theme) {
            state.notify.test_requested = true;
            let now_t = ui.ctx().input(|i| i.time);
            ui.ctx().data_mut(|d| d.insert_temp(shown_at, now_t));
        }
        let pressed = ui.ctx().data(|d| d.get_temp::<f64>(shown_at));
        if pressed.is_some_and(|at| ui.ctx().input(|i| i.time) - at < 12.0) {
            ui.label(RichText::new(t("notifications.test_shown")).size(theme.font_size_small).color(theme.text_secondary()));
        }
        widgets::setting_hint(ui, theme, hint, t("notifications.test_hint"));

        // The notification area (rung 2, part 2): the tray icon, and closing into it.
        let s = &mut state.settings.notifications;
        dirty |= with_hint(ui, "notifications.tray_hint", |ui| widgets::toggle(ui, theme, t("notifications.tray"), &mut s.tray));
        widgets::setting_hint(ui, theme, hint, t("notifications.tray_hint"));
        // A desktop's own rule, said where a person meets it (CLAUDE.md, "Saying what we will not
        // build"): GNOME shows no tray icons without its AppIndicator extension.
        if cfg!(target_os = "linux") {
            widgets::setting_hint(ui, theme, hint, t("notifications.tray_linux_note"));
        }
        ui.add_enabled_ui(s.tray, |ui| {
            dirty |= with_hint(ui, "notifications.close_to_tray_hint", |ui| widgets::toggle(ui, theme, t("notifications.close_to_tray"), &mut s.close_to_tray));
        });
        widgets::setting_hint(ui, theme, hint, t("notifications.close_to_tray_hint"));
        if !s.tray && s.close_to_tray {
            ui.label(RichText::new(t("notifications.close_to_tray_needs_tray")).size(theme.font_size_small).color(theme.text_muted()));
        }
        if dirty {
            state.settings_dirty = true;
        }
    });

    // ── What notifies you (kept on the server) ──
    widgets::subsection_header(ui, theme, accent, t("notifications.synced_heading"), "");
    let connected = state.ws_client.as_ref().is_some_and(|c| c.is_connected());
    let mut changed = false;
    widgets::card(ui, theme, |ui| {
        widgets::setting_hint(ui, theme, hint, t("notifications.synced_note"));
        if !connected {
            ui.label(RichText::new(t("notifications.not_connected")).size(theme.font_size_small).color(theme.text_muted()));
        } else if !state.notif_prefs_loaded {
            ui.label(RichText::new(t("notifications.loading")).size(theme.font_size_small).color(theme.text_muted()));
        }
        // Until the server has answered there is nothing true to show or change (synced_frame).
        ui.add_enabled_ui(connected && state.notif_prefs_loaded, |ui| {
            changed |= with_hint(ui, "notifications.kind_dm_hint", |ui| widgets::toggle(ui, theme, t("notifications.kind_dm"), &mut state.notif_dm_enabled));
            widgets::setting_hint(ui, theme, hint, t("notifications.kind_dm_hint"));
            changed |= with_hint(ui, "notifications.kind_mention_hint", |ui| widgets::toggle(ui, theme, t("notifications.kind_mention"), &mut state.notif_mentions_enabled));
            widgets::setting_hint(ui, theme, hint, t("notifications.kind_mention_hint"));
            changed |= with_hint(ui, "notifications.kind_task_hint", |ui| widgets::toggle(ui, theme, t("notifications.kind_task"), &mut state.notif_tasks_enabled));
            widgets::setting_hint(ui, theme, hint, t("notifications.kind_task_hint"));

            // Quiet hours, by the hour (the website's choice too); both ends set, or neither.
            let window = n::quiet::window(state.notif_dnd_start.as_deref(), state.notif_dnd_end.as_deref());
            let mut on = window.is_some();
            if with_hint(ui, "notifications.quiet_hint", |ui| widgets::toggle(ui, theme, t("notifications.quiet"), &mut on)) {
                changed = true;
                if on {
                    state.notif_dnd_start = Some(n::quiet::format_clock(22 * 60));
                    state.notif_dnd_end = Some(n::quiet::format_clock(7 * 60));
                } else {
                    state.notif_dnd_start = None;
                    state.notif_dnd_end = None;
                }
            }
            widgets::setting_hint(ui, theme, hint, t("notifications.quiet_hint"));
            // The operator, 2026-11-01: calls wait during quiet hours unless the person added the
            // caller to the always-allow list, "and we should include that information in the quiet
            // hours gui element": said here, always, in plain words.
            ui.label(RichText::new(t("notifications.quiet_allow_note")).size(theme.font_size_small).color(theme.text_secondary()));
            if let Some((start, end)) = n::quiet::window(state.notif_dnd_start.as_deref(), state.notif_dnd_end.as_deref()) {
                for (label, key, id, value, is_start) in [
                    (t("notifications.quiet_from"), "notifications.quiet_from_hint", "notify_quiet_from", start, true),
                    (t("notifications.quiet_to"), "notifications.quiet_to_hint", "notify_quiet_to", end, false),
                ] {
                    widgets::form_row(ui, theme, label, |ui| {
                        let mut hour = value / 60;
                        let before = hour;
                        egui::ComboBox::from_id_salt(id)
                            .selected_text(n::quiet::format_clock(hour * 60))
                            .show_ui(ui, |ui| {
                                for h in 0..24u16 {
                                    ui.selectable_value(&mut hour, h, n::quiet::format_clock(h * 60)).hint(key);
                                }
                            })
                            .response
                            .hint(key);
                        if hour != before {
                            changed = true;
                            let v = Some(n::quiet::format_clock(hour * 60));
                            if is_start {
                                state.notif_dnd_start = v;
                            } else {
                                state.notif_dnd_end = v;
                            }
                        }
                    });
                }
                // What still comes through quiet hours, read as one list: the people always allowed
                // (rung 3, kept on my devices), then the two exceptions (rung 2, synced).
                draw_allow_list(ui, theme, state);
                changed |= with_hint(ui, "notifications.quiet_friends_hint", |ui| {
                    widgets::toggle(ui, theme, t("notifications.quiet_friends"), &mut state.notif_quiet_friends)
                });
                widgets::setting_hint(ui, theme, hint, t("notifications.quiet_friends_hint"));
                changed |= with_hint(ui, "notifications.quiet_mentions_hint", |ui| {
                    widgets::toggle(ui, theme, t("notifications.quiet_mentions"), &mut state.notif_quiet_mentions)
                });
                widgets::setting_hint(ui, theme, hint, t("notifications.quiet_mentions_hint"));
            }
        });
    });
    if changed && connected {
        send_synced(state);
    }

    // ── Your home (rung 3: the game events, on this computer) ──
    widgets::subsection_header(ui, theme, accent, t("notifications.events_heading"), "");
    widgets::card(ui, theme, |ui| draw_events(ui, theme, state));

    // ── Muted ──
    widgets::subsection_header(ui, theme, accent, t("notifications.muted_heading"), "");
    widgets::card(ui, theme, |ui| draw_muted(ui, theme, state, now));
}

/// The always-allow list (rung 3): everyone on it, each with Remove, or "Nobody yet." Adding is in a
/// conversation's menu (the note above says so).
fn draw_allow_list(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new(t("notifications.quiet_allow_heading")).color(theme.text_primary()));
    let people: Vec<String> = state.settings.notifications.allow_list(&state.profile_public_key).map(|l| l.allowed().map(str::to_string).collect()).unwrap_or_default();
    if people.is_empty() {
        ui.label(RichText::new(t("notifications.quiet_allow_empty")).size(theme.font_size_small).color(theme.text_muted()));
    }
    for key in people {
        let name = crate::engine::dm::dm_display_name(state, &key);
        ui.horizontal(|ui| {
            ui.label(RichText::new(name).color(theme.text_primary()));
            if widgets::Button::secondary(t("notifications.quiet_allow_remove")).hint("notifications.quiet_allow_remove_hint").show(ui, theme) {
                crate::engine::quiet_allow::set(state, &key, false);
            }
        });
    }
}

/// "Your home" (rung 3): a switch per game event the simulation raises, on this computer.
fn draw_events(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let hint = state.settings.hint_display;
    let file = n::data::get();
    widgets::setting_hint(ui, theme, hint, t("notifications.events_note"));
    for e in file.game_events.iter().filter(|e| e.built) {
        let mut on = state.settings.notifications.event_on(file, &e.id);
        if with_hint(ui, e.hint.as_str(), |ui| widgets::toggle(ui, theme, t_dyn(&e.label), &mut on)) {
            state.settings.notifications.set_event(file, &e.id, on);
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, t_dyn(&e.hint));
    }
}

/// The muted list: Mute everything, then each muted server and room, each with Unmute.
fn draw_muted(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, now: i64) {
    let hint = state.settings.hint_display;
    let mutes = state.settings.notifications.mutes.clone();
    match mutes.everything_muted(now) {
        Some(until) => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(t("notifications.muted_everything")).color(theme.text_primary()));
                ui.label(RichText::new(until_words(until)).size(theme.font_size_small).color(theme.text_secondary()));
                if widgets::Button::secondary(t("notifications.unmute")).hint("notifications.unmute_hint").show(ui, theme) {
                    state.settings.notifications.mutes.unmute_everything();
                    state.settings_dirty = true;
                }
            });
        }
        None => {
            let menu = ui.menu_button(t("notifications.mute_everything"), |ui| mute_choices(ui));
            menu.response.hint("notifications.mute_everything_hint");
            if let Some(seconds) = menu.inner.flatten() {
                state.settings.notifications.mutes.mute_everything(now, seconds);
                state.settings_dirty = true;
            }
        }
    }
    widgets::setting_hint(ui, theme, hint, t("notifications.mute_everything_hint"));

    let live = |m: &std::collections::BTreeMap<String, i64>| -> Vec<(String, i64)> {
        m.iter().filter(|(_, u)| n::mutes::active(**u, now)).map(|(k, u)| (k.clone(), *u)).collect()
    };
    let (servers, rooms, people) = (live(&mutes.servers), live(&mutes.rooms), live(&mutes.people));
    if servers.is_empty() && rooms.is_empty() && people.is_empty() {
        ui.label(RichText::new(t("notifications.muted_none")).size(theme.font_size_small).color(theme.text_muted()));
    }
    for (conversation, until) in people {
        let key = conversation.strip_prefix("dm:").unwrap_or(&conversation);
        let words = tf("notifications.muted_person", &[("name", &crate::engine::dm::dm_display_name(state, key))]);
        ui.horizontal(|ui| {
            ui.label(RichText::new(words).color(theme.text_primary()));
            ui.label(RichText::new(until_words(until)).size(theme.font_size_small).color(theme.text_secondary()));
            if widgets::Button::secondary(t("notifications.unmute")).hint("notifications.unmute_hint").show(ui, theme) {
                state.settings.notifications.mutes.unmute_person(&conversation);
                state.settings_dirty = true;
            }
        });
    }
    for (url, until) in servers {
        let name = server_name(state, &url);
        ui.horizontal(|ui| {
            ui.label(RichText::new(name).color(theme.text_primary()));
            ui.label(RichText::new(until_words(until)).size(theme.font_size_small).color(theme.text_secondary()));
            if widgets::Button::secondary(t("notifications.unmute")).hint("notifications.unmute_hint").show(ui, theme) {
                state.settings.notifications.mutes.unmute_server(&url);
                state.settings_dirty = true;
            }
        });
    }
    for (key, until) in rooms {
        let (url, room) = key.split_once('|').unwrap_or((key.as_str(), ""));
        let words = tf("notifications.muted_room", &[("room", &room), ("server", &server_name(state, url))]);
        ui.horizontal(|ui| {
            ui.label(RichText::new(words).color(theme.text_primary()));
            ui.label(RichText::new(until_words(until)).size(theme.font_size_small).color(theme.text_secondary()));
            if widgets::Button::secondary(t("notifications.unmute")).hint("notifications.unmute_hint").show(ui, theme) {
                state.settings.notifications.mutes.unmute_room(url, room);
                state.settings_dirty = true;
            }
        });
    }
    draw_room_levels(ui, theme, state);
    // The unfinished-software warning bar, muted from its own Mute or from here (2026-11-02).
    ui.separator();
    crate::gui::bottom_strip::mute::settings_row(ui, theme, state);
}

/// The words of a room level: its label, by the data file's `levels`.
pub(crate) fn level_words(level: n::Level) -> &'static str {
    n::data::get().levels.iter().find(|l| l.id == level).map(|l| t_dyn(&l.label)).unwrap_or("")
}

/// The rooms set to something other than mentions (every line, or nothing), each with Back to
/// mentions: a room set to nothing is as quiet as one muted until unmuted, so it is listed here
/// where the mutes are, never left to be found only from its right-click.
fn draw_room_levels(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if state.settings.notifications.room_levels.is_empty() {
        return;
    }
    let hint = state.settings.hint_display;
    ui.separator();
    ui.label(RichText::new(t("notifications.levels_heading")).size(theme.font_size_small).color(theme.text_secondary()).strong());
    let levels: Vec<(String, n::Level)> = state.settings.notifications.room_levels.iter().map(|(k, l)| (k.clone(), *l)).collect();
    for (key, level) in levels {
        let (url, room) = key.split_once('|').unwrap_or((key.as_str(), ""));
        let words = tf("notifications.room_level_row", &[("room", &room), ("server", &server_name(state, url)), ("level", &level_words(level))]);
        ui.horizontal(|ui| {
            ui.label(RichText::new(words).color(theme.text_primary()));
            if widgets::Button::secondary(t("notifications.level_reset")).hint("notifications.level_reset_hint").show(ui, theme) {
                state.settings.notifications.set_room_level(url, room, n::Level::Mentions);
                state.settings_dirty = true;
            }
        });
    }
    widgets::setting_hint(ui, theme, hint, t("notifications.level_reset_hint"));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The person's switches are written only once their server has said what it holds
    /// (2026-10-30): before that the app holds only the defaults (every switch on, no quiet
    /// hours), and sending those would undo what the person chose on another device. Every
    /// write from this device goes through `synced_frame` (Settings and the chat's DM cog).
    ///
    /// Seen red 2026-10-30: `synced_frame` built a frame from the defaults before the answer.
    #[test]
    fn nothing_is_written_until_the_server_answered() {
        let mut state = GuiState::default();
        assert!(!state.notif_prefs_loaded);
        assert!(synced_frame(&state).is_none(), "the defaults must never reach the server");
        state.notif_prefs_loaded = true;
        state.notif_mentions_enabled = false;
        state.notif_dnd_start = Some("22:00".into());
        state.notif_dnd_end = Some("07:00".into());
        let frame: serde_json::Value = serde_json::from_str(&synced_frame(&state).expect("loaded: a frame")).unwrap();
        assert_eq!(frame["type"], "update_notification_prefs");
        assert_eq!(frame["mentions"], false);
        assert_eq!(frame["dnd_start"], "22:00");
        // Quiet hours off travel as null, which the relay reads as "clear them".
        state.notif_dnd_start = None;
        state.notif_dnd_end = None;
        let off: serde_json::Value = serde_json::from_str(&synced_frame(&state).unwrap()).unwrap();
        assert!(off.get("dnd_start").is_some_and(|v| v.is_null()), "{off}");
        // The quiet-hours exceptions (rung 2) travel with the rest, so every server holds them.
        // Seen red 2026-10-31 with `quiet_mentions` left out of the frame.
        state.notif_quiet_mentions = true;
        let ex: serde_json::Value = serde_json::from_str(&synced_frame(&state).unwrap()).unwrap();
        assert_eq!((ex["quiet_friends"].as_bool(), ex["quiet_mentions"].as_bool()), (Some(false), Some(true)), "{ex}");
    }
}

//! "Your settings for <name>": the per-person controls on another person's profile card
//! (docs/design/per-person-controls.md, section 4; the operator, 2026-11-03: "We need to add the
//! various controls users can use on other users to the profile page. Like we need to set file
//! sharing limits, allowed to call during quiet hours, etc.").
//!
//! One plain section, drawn by `modals.rs` `draw_user_modal` below the person's primary actions
//! and above Report and Block, apart from the Moderation and Admin tools. Each control is a label
//! line above its control, so the section works at the card's narrowest. Rung 1:
//!
//! - Notifications from them: Everything / Only calls, mentions and replies / Nothing (the data
//!   file's `person_levels`; THE rule reads it, `notifications::decide`).
//! - Calls and messages come through quiet hours: the always-allow list (`engine/quiet_allow.rs`).
//! - Files from them: Use my setting / Always ask / Up to N MB without asking / Never
//!   (`net/person_controls.rs` `file_verdict`, read by the file's plan and a big file's question).
//! - What a friend may do: their Message, Call and Trade ticks, the same as Settings > Safety's
//!   "People I choose" (`engine/reach.rs` `set_tick`, so the protected setup's PIN still guards
//!   them). Anyone else gets one line saying what applies.
//! - Their conversation's mute, while one holds, with Unmute (`notifications::mutes`).
//!
//! Every word is a key in `data/i18n/en.json`, every control has its hint. Kept on this device
//! (rung 2 of the design carries the record between the person's devices).

use egui::RichText;

use humanity_core::i18n::{t, t_dyn, tf};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Hint};
use crate::gui::GuiState;
use crate::net::person_controls::{Controls, FilesChoice, Notices, PeopleControls, FILES_MB_RANGE};
use crate::net::reach::ReachKind;

/// Does the card show the section for `key`: someone else (not a bot), with an identity unlocked
/// to keep the choices under?
pub(crate) fn applies(state: &GuiState, key: &str) -> bool {
    !key.is_empty() && !state.profile_public_key.is_empty() && !key.eq_ignore_ascii_case(&state.profile_public_key) && !key.starts_with("bot_")
}

/// Keep `c` as this identity's choice for `key` (the defaults as no entry), to be saved.
fn keep(people: &mut PeopleControls, me: &str, key: &str, c: Controls) {
    people.set(me, key, c);
}

/// The section, for the person `key` shown as `name`.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, key: &str, name: &str) {
    if !applies(state, key) {
        return;
    }
    let me = state.profile_public_key.clone();
    let mut c = state.settings.person_controls.get(&me, key);
    let before = c;

    ui.add_space(theme.spacing_md);
    ui.separator();
    ui.add_space(theme.spacing_xs);
    ui.label(RichText::new(tf("person.heading", &[("name", &name)])).size(theme.font_size_small).color(theme.text_secondary()).strong())
        .hint("person.heading_hint");
    ui.add_space(theme.spacing_xs);

    // ── Notifications from them ──
    caption(ui, theme, &tf("person.notices", &[("name", &name)]));
    let file = crate::notifications::data::get();
    let current = file.person_levels.iter().find(|l| l.id.id() == c.notices.id());
    let shown = current.map(|l| t_dyn(&l.label)).unwrap_or_default();
    let combo = egui::ComboBox::from_id_salt(("person_notices", key))
        .selected_text(shown)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for choice in &file.person_levels {
                if ui.selectable_label(c.notices.id() == choice.id.id(), t_dyn(&choice.label)).hint_dyn(&choice.hint).clicked() {
                    c.notices = Notices::from_id(choice.id.id());
                }
            }
        });
    combo.response.hint("person.notices_hint");

    // ── Calls and messages through quiet hours (the always-allow list) ──
    let mut allowed = crate::engine::quiet_allow::allows(state, key);
    if tick(ui, theme, "person.quiet_allow", "person.quiet_allow_hint", &mut allowed) {
        crate::engine::quiet_allow::set(state, key, allowed);
    }
    if c.notices == Notices::Nothing {
        note(ui, theme, t("person.quiet_allow_nothing"));
    }

    // ── Files from them ──
    ui.add_space(theme.spacing_xs);
    caption(ui, theme, &tf("person.files", &[("name", &name)]));
    let combo = egui::ComboBox::from_id_salt(("person_files", key))
        .selected_text(t(c.files.label_key()))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for choice in FilesChoice::ALL {
                if ui.selectable_label(c.files == choice, t(choice.label_key())).hint(choice.hint_key()).clicked() {
                    c.files = choice;
                }
            }
        });
    combo.response.hint("person.files_hint");
    if c.files == FilesChoice::UpTo {
        let mut mb = c.mb();
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut mb).range(FILES_MB_RANGE.0..=FILES_MB_RANGE.1).speed(1.0)).hint("person.files_mb_hint");
            ui.label(RichText::new(t("person.files_mb_unit")).size(theme.font_size_small).color(theme.text_secondary()));
        });
        c.files_mb = mb;
    }
    if c.files != FilesChoice::Mine {
        note(ui, theme, t(c.files.hint_key()));
    }

    // ── What a friend may do (their pass's ticks) ──
    ui.add_space(theme.spacing_xs);
    draw_ticks(ui, theme, state, key, name);

    // ── Their conversation's mute, while one holds ──
    let conversation = format!("dm:{key}");
    if let Some(until) = super::mute_menu::person_mute(state, &conversation) {
        ui.add_space(theme.spacing_xs);
        ui.horizontal_wrapped(|ui| {
            let words = tf("notifications.muted_notice", &[("until", &crate::gui::pages::settings::notifications::until_words(until))]);
            ui.label(RichText::new(words).size(theme.font_size_small).color(theme.text_secondary()));
            if widgets::Button::ghost(t("notifications.unmute_person")).size(widgets::ButtonSize::Small).hint("notifications.unmute_hint").show(ui, theme) {
                state.settings.notifications.mutes.unmute_person(&conversation);
                state.settings_dirty = true;
            }
        });
    }

    ui.add_space(theme.spacing_xs);
    note(ui, theme, t("person.kept_here"));

    if c != before {
        keep(&mut state.settings.person_controls, &me, key, c);
        state.settings_dirty = true;
    }
}

/// The friend's Message, Call and Trade ticks, or the line for someone who is not a friend.
fn draw_ticks(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, key: &str, name: &str) {
    let Some(store) = state.dm_store.as_ref() else { return };
    let friend = store.people_to_choose().iter().any(|k| k.eq_ignore_ascii_case(key));
    if !friend {
        note(ui, theme, t("person.not_friend"));
        return;
    }
    // A friend marked "changed on my other device" is drawn with nothing ticked (10n N6), as on
    // Settings > Safety: a tick here gives exactly what is ticked.
    let ticks = if store.changed_elsewhere(key) { crate::net::reach::FriendTicks::NONE } else { store.ticks(key) };
    caption(ui, theme, &tf("person.may", &[("name", &name)]));
    let mut toggled: Option<(ReachKind, bool)> = None;
    ui.horizontal_wrapped(|ui| {
        for kind in ReachKind::ALL {
            let mut on = ticks.get(kind);
            let (label, hint) = match kind {
                ReachKind::Message => ("person.may_message", "person.may_message_hint"),
                ReachKind::Call => ("person.may_call", "person.may_call_hint"),
                ReachKind::Trade => ("person.may_trade", "person.may_trade_hint"),
            };
            if tick(ui, theme, label, hint, &mut on) {
                toggled = Some((kind, on));
            }
            ui.add_space(theme.spacing_sm);
        }
    });
    note(ui, theme, t("person.may_note"));
    if let Some((kind, on)) = toggled {
        crate::engine::reach::set_tick(state, key, kind, on);
    }
}

/// A box and its words, both toggling it, both carrying its hint. True when it changed. The words
/// wrap beside the box, so a long one never runs past a narrow card's edge.
fn tick(ui: &mut egui::Ui, theme: &Theme, label: &'static str, hint: &'static str, value: &mut bool) -> bool {
    ui.horizontal_top(|ui| {
        let mut changed = widgets::with_hint(ui, hint, |ui| widgets::custom_checkbox(ui, theme, value));
        let words = RichText::new(t(label)).size(theme.font_size_body).color(theme.text_primary());
        if ui.add(egui::Label::new(words).wrap().sense(egui::Sense::click())).hint(hint).clicked() {
            *value = !*value;
            changed = true;
        }
        changed
    })
    .inner
}

/// A control's label line.
fn caption(ui: &mut egui::Ui, theme: &Theme, words: &str) {
    ui.label(RichText::new(words).size(theme.font_size_small).color(theme.text_muted()));
}

/// A small explaining line under a control.
fn note(ui: &mut egui::Ui, theme: &Theme, words: &str) {
    ui.add(egui::Label::new(RichText::new(words).size(theme.font_size_small).color(theme.text_muted())).wrap());
}

#[cfg(test)]
#[path = "person_controls_tests.rs"]
mod tests;

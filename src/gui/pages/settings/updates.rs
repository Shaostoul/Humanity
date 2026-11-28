//! Settings > Updates (moved out of settings.rs and put into words by key on 2026-11-27, when the
//! phone app began updating itself; src/updater.rs, src/updater/phone.rs).
//!
//! - **Look for updates**: Always (at start and once an hour, the default) or Never by itself.
//!   Kept on this device (config.json `update_channel`) since that day; before, it went back to
//!   Always at every start. Check now looks once whatever it says.
//! - **What the updater has to say, and its buttons**: the same sentence and the same buttons as
//!   the update line at the bottom of every page (gui/bottom_strip/update_offer.rs `sentence`,
//!   `detail`, `actions`), so the two never say different things.
//! - **Available versions**: a desktop can install any listed version; the phone app offers only
//!   the newest, because Android does not install an older version over a newer one (a platform
//!   holder's rule, Google's), and the page says so.

use egui::{CornerRadius, RichText};

use humanity_core::i18n::{t, tf};
use crate::gui::bottom_strip::update_offer;
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Hint};
use crate::gui::{GuiState, VERSION};
use crate::updater::{UpdateChannel, UpdateState, UpdateTarget};

pub(crate) fn draw_updates_content(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let hint = state.settings.hint_display;
    let phone = state.updater.target == UpdateTarget::Phone;
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new(tf("app_update.current_version", &[("version", &VERSION)])).strong());
        ui.add_space(theme.spacing_sm);

        // Look for updates: kept with the settings (config.rs `update_channel`).
        ui.label(RichText::new(t("app_update.look")).color(theme.text_secondary()));
        let before = state.updater.channel;
        ui.radio_value(&mut state.updater.channel, UpdateChannel::AlwaysLatest, t("app_update.look_latest"))
            .hint("app_update.look_latest_hint");
        ui.radio_value(&mut state.updater.channel, UpdateChannel::Disabled, t("app_update.look_off"))
            .hint("app_update.look_off_hint");
        if state.updater.channel != before {
            state.settings_dirty = true;
        }
        widgets::setting_hint(ui, theme, hint, t("app_update.look_note"));

        // What the updater has to say: the update line's own words.
        ui.add_space(theme.spacing_sm);
        let color = match &state.updater.state {
            UpdateState::Available { .. } => theme.accent(),
            UpdateState::Error(_) | UpdateState::Failed { .. } => theme.danger(),
            UpdateState::Ready { .. } => theme.success(),
            _ => theme.text_secondary(),
        };
        ui.label(RichText::new(update_offer::sentence(&state.updater)).color(color));
        if let Some(d) = update_offer::detail(&state.updater) {
            ui.label(RichText::new(d).size(theme.font_size_small).color(theme.text_muted()));
        }
        ui.add_space(theme.spacing_sm);

        ui.horizontal_wrapped(|ui| {
            if widgets::Button::primary(t("app_update.check_now")).hint("app_update.check_now_hint").show(ui, theme) {
                state.updater.check_now();
            }
            update_offer::actions(ui, theme, state, false);
        });

        if let UpdateState::Downloading { progress, .. } = &state.updater.state {
            ui.add_space(theme.spacing_sm);
            widgets::progress_bar(ui, theme, *progress, None);
        }

        // What changed in the offered version, as its release notes say.
        if let UpdateState::Available { ref release_notes, .. } = &state.updater.state {
            if !release_notes.is_empty() {
                ui.add_space(theme.spacing_md);
                ui.label(RichText::new(t("app_update.notes")).color(theme.text_secondary()).strong());
                ui.add_space(theme.spacing_xs);
                egui::Frame::NONE
                    .fill(theme.bg_sidebar_dark())
                    .corner_radius(CornerRadius::same(4))
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.label(RichText::new(release_notes).color(theme.text_muted()).size(theme.font_size_small));
                    });
            }
        }
    });

    ui.add_space(theme.spacing_md);

    // Version picker: a desktop can put any listed version in place; the phone app cannot go back.
    widgets::card_with_header(ui, theme, t("app_update.versions"), |ui| {
        if phone {
            ui.label(RichText::new(t("app_update.versions_phone")).color(theme.text_muted()));
            return;
        }
        let versions = state.updater.available_versions();
        if versions.is_empty() {
            ui.label(RichText::new(t("app_update.versions_none")).color(theme.text_muted()));
            return;
        }
        for (tag, date, is_current) in &versions {
            ui.horizontal(|ui| {
                let label = if *is_current {
                    RichText::new(tf("app_update.versions_current", &[("version", tag)])).strong().color(theme.success())
                } else {
                    RichText::new(tag).color(theme.text_primary())
                };
                ui.label(label);
                ui.label(RichText::new(date).small().color(theme.text_muted()));
                if !is_current
                    && widgets::Button::secondary(t("app_update.versions_install")).hint("app_update.versions_install_hint").show(ui, theme)
                {
                    state.updater.download_version(tag);
                }
            });
        }
    });
}

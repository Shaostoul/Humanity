//! The update line (2026-11-27): a new version of HumanityOS offered at the bottom of every app
//! page, on a desktop and on the phone alike, never in a window that interrupts.
//!
//! The operator asked on 2026-11-22 how to update the phone app; the answer was a release page a
//! person new to tech had to find. Now the updater notices a new release (src/updater.rs, the same
//! check on both: at start and once an hour while "Look for updates" is on) and this line says so,
//! with the version and its size, and Download. Then it shows the download's progress, then on a
//! phone Install (Android asks the person to confirm, src/updater/phone.rs) and on a desktop
//! Restart to apply; a download that stopped says why and offers Try again. Not now hides an offer
//! for this run (Settings > Updates still has it). A failed check does not show here: a phone that
//! started offline is not news on every page (Settings says it). The desktop's world view keeps its
//! own small notice (pages/hud.rs, the same words by key); the line is for the pages.
//!
//! WHERE: in the bottom strip (bottom_strip.rs), above the unfinished-software warning and below
//! the chat footer, so it never covers either and its height counts in [`super::height`].
//!
//! THE WORDS are the same keys Settings > Updates shows (`app_update.*`, data/i18n/en.json):
//! [`sentence`] and [`actions`] are shared, so the line and the page never say different things.
//! The website has no line: a page served by the website is always the newest, and its Download
//! page says the phone app updates itself.
//!
//! COST: nothing when there is nothing to offer (one match on the updater's state). The line
//! hands the frame's context to the updater, so a background check or download wakes a phone that
//! draws only on change (engine/redraw.rs).
//!
//! Colours are theme tokens: the card ground (`bg_card`), its top edge in `accent`, the words in
//! `text_primary`, a stopped download in `warning`, its detail in `text_muted`.

use egui::{RichText, Stroke};

use humanity_core::i18n::{t, tf};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;
use crate::updater::{UpdateState, UpdateTarget, Updater};

/// The line's panel, by id.
const PANEL: &str = "hos_update_offer";

/// Draw the line when the updater has something to say ([`Updater::offer_shown`]). Returns its
/// height in points, 0 when it did not draw.
pub(super) fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) -> f32 {
    state.updater.set_waker(ctx);
    if !state.updater.offer_shown() {
        return 0.0;
    }
    let panel = egui::TopBottomPanel::bottom(PANEL)
        .frame(egui::Frame::NONE.fill(theme.bg_card()).inner_margin(egui::Margin::symmetric(8, 4)))
        // Its own edge in the accent colour, not the panel's grey separator.
        .show_separator_line(false)
        .resizable(false)
        .show(ctx, |ui| {
            let edge = ui.max_rect().expand2(egui::vec2(8.0, 4.0));
            ui.painter().hline(edge.x_range(), edge.top() + 0.5, Stroke::new(1.0_f32, theme.accent()));
            // The words, then the buttons; on a narrow phone the buttons wrap under the words.
            ui.horizontal_wrapped(|ui| {
                let color = if matches!(state.updater.state, UpdateState::Failed { .. }) { theme.warning() } else { theme.text_primary() };
                ui.label(RichText::new(sentence(&state.updater)).color(color));
                actions(ui, theme, state, true);
            });
            if let Some(d) = detail(&state.updater) {
                ui.label(RichText::new(d).size(theme.font_size_small).color(theme.text_muted()));
            }
            if let UpdateState::Downloading { progress, .. } = state.updater.state {
                widgets::progress_bar(ui, theme, progress, None);
            }
        });
    panel.response.rect.height()
}

/// What the updater has to say, in the person's words: the offer with its version and size, the
/// download's progress, Install or Restart, why a download stopped, or (in Settings) whether it
/// has looked.
pub(crate) fn sentence(u: &Updater) -> String {
    match &u.state {
        UpdateState::Idle => t("app_update.status_idle").to_string(),
        UpdateState::Checking => t("app_update.status_checking").to_string(),
        UpdateState::UpToDate => t("app_update.status_up_to_date").to_string(),
        UpdateState::Error(_) => t("app_update.status_check_failed").to_string(),
        UpdateState::Available { version, asset_size, .. } => {
            let size = widgets::file_browser::human_size(*asset_size);
            tf("app_update.offer", &[("version", version), ("size", &size)])
        }
        UpdateState::Downloading { version, progress } => {
            let percent = (progress.clamp(0.0, 1.0) * 100.0).round() as i32;
            tf("app_update.downloading", &[("version", version), ("percent", &percent)])
        }
        UpdateState::Ready { version } => match u.target {
            UpdateTarget::Phone => tf("app_update.ready_phone", &[("version", version)]),
            UpdateTarget::Desktop => tf("app_update.ready_desktop", &[("version", version)]),
        },
        UpdateState::Failed { key, .. } => t(*key).to_string(),
    }
}

/// The technical part under the sentence, when there is one: why a check or a download failed.
pub(crate) fn detail(u: &Updater) -> Option<&str> {
    match &u.state {
        UpdateState::Error(e) => Some(e.as_str()),
        UpdateState::Failed { detail, .. } if !detail.is_empty() => Some(detail.as_str()),
        _ => None,
    }
}

/// The buttons for the updater's state: Download for an offer (and Not now, on the line), Install
/// on a phone or Restart to apply on a desktop once it is downloaded, Try again after a stop.
/// Settings > Updates draws the same ones beside its Check now (`on_line` false).
pub(crate) fn actions(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, on_line: bool) {
    let phone = state.updater.target == UpdateTarget::Phone;
    match state.updater.state.clone() {
        UpdateState::Available { version, .. } => {
            let hint = if phone { "app_update.download_hint_phone" } else { "app_update.download_hint_desktop" };
            if widgets::Button::primary(t("app_update.download")).hint(hint).show(ui, theme) {
                state.updater.download_version(&version);
            }
            if on_line && widgets::Button::ghost(t("app_update.not_now")).hint("app_update.not_now_hint").show(ui, theme) {
                state.updater.hide_offer();
            }
        }
        UpdateState::Ready { .. } if phone => {
            if widgets::Button::primary(t("app_update.install")).hint("app_update.install_hint").show(ui, theme) {
                state.updater.install_downloaded();
            }
        }
        UpdateState::Ready { .. } => {
            if widgets::Button::primary(t("app_update.restart")).hint("app_update.restart_hint").show(ui, theme) {
                state.updater.restart_into_update();
                state.quit_requested = true;
            }
        }
        UpdateState::Failed { version, .. } if !version.is_empty() => {
            if widgets::Button::primary(t("app_update.try_again")).hint("app_update.try_again_hint").show(ui, theme) {
                state.updater.download_version(&version);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "update_offer_tests.rs"]
mod tests;

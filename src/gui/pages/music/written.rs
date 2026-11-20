//! The Music page's WRITTEN PIECES (docs/design/score-player.md section 10): music written down as MEI
//! (data/music/scores/index.ron lists it), played on the game's own instruments at the person's
//! tuning. Play keeps the piece's id in the person's settings (`MusicSettings::written`), so it plays
//! everywhere, page open or not, until Stop clears it and what played before plays again: the one
//! place that chooses the music stays this page (docs/design/music-page.md section 4). The page plays
//! nothing itself: the game's one score takes the piece over at its next bar line.

use egui::{Color32, RichText};
use humanity_core::i18n::t;

use super::score_words;
use crate::gui::theme::Theme;
use crate::gui::widgets::Hint;
use crate::gui::{widgets, GuiState};

/// The written-pieces list: each piece's title and composer, Play or Stop, and what it is doing now.
pub(super) fn written(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: Color32) {
    let hint = state.settings.hint_display;
    let lib = state.music_dev.readout.lib.clone();
    let playing = state.music_dev.readout.written.clone();
    let failed = state.music_dev.readout.written_error.clone();
    let mut dirty = false;
    widgets::subsection_header(ui, theme, accent, t("music_page.written"), t("music_page.written_note"));
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new(t("music_page.written_intro")).color(theme.text_secondary()));
        ui.add_space(theme.spacing_xs);
        let scores = lib.as_ref().map(|l| l.scores.clone()).unwrap_or_default();
        if scores.is_empty() {
            ui.label(RichText::new(t("music_page.written_none")).color(theme.text_muted()));
        }
        let m = &mut state.settings.music;
        for e in &scores {
            let (title, composer) = score_words(e);
            let chosen = m.written == e.id;
            ui.horizontal_wrapped(|ui| {
                if chosen {
                    if ui.button(t("music_page.written_stop")).hint("music_page.written_stop_hint").clicked() {
                        m.written.clear();
                        dirty = true;
                    }
                } else if ui.button(t("music_page.written_play")).hint("music_page.written_play_hint").clicked() {
                    m.written = e.id.clone();
                    dirty = true;
                }
                ui.label(RichText::new(title).color(theme.text_primary()).strong());
                if !composer.is_empty() {
                    ui.label(RichText::new(t("music_page.written_by").replace("{{composer}}", &composer)).color(theme.text_secondary()));
                }
                if chosen {
                    let now = if playing.as_deref() == Some(e.id.as_str()) { t("music_page.written_playing") } else { t("music_page.written_starting") };
                    ui.label(RichText::new(now).color(accent));
                }
            });
            // Where the piece came from, beside it (the finding's courtesy credit: the edition, and the
            // typesetter who placed the file in the public domain).
            if let Some(credit) = super::credit_words(e) {
                ui.label(RichText::new(credit).color(theme.text_muted()).size(theme.font_size_small));
            }
            if let Some((id, why)) = &failed {
                if *id == e.id {
                    ui.label(RichText::new(t("music_page.written_error").replace("{{why}}", why)).color(theme.warning()));
                }
            }
        }
        widgets::setting_hint(ui, theme, hint, t("music_page.written_hint"));
    });
    if dirty {
        state.settings_dirty = true;
    }
}

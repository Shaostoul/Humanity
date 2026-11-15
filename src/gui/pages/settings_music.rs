//! Settings > Music (docs/design/music.md, sections 7a, 10, 11 and 15): how the game's live music
//! sounds. Every word by key, every control with its hint. The values live in
//! `SettingsState::music` (humanity_music::MusicSettings) and are saved in config.json as one `music`
//! object.
//!
//! WHAT plays is chosen in one place, the Music page (2026-11-14, docs/design/music-page.md section
//! 4; the operator: "When I go to the settings it plays one music and when I go to the music page it
//! goes to another. We should have one place ideally"). So this section opens with what is playing
//! now, in the Music page's own words (`music::now_playing_words`, one function for both pages), and
//! a Choose the music button that opens the page; the genre, the blend, places and "A new piece"
//! moved there, and no Settings section draws a genre control
//! (`there_is_one_genre_control_and_no_settings_page_draws_one`). What stays here is how the music
//! sounds: on, on the app's pages, the tuning, the ship's hum and the optional tones. How loud it is
//! (Music Volume, Even out loudness, the instrument packs) is under Settings > Audio.
//!
//! The words about tuning, tones and the hum are exactly the design's (section 15): no health claim
//! anywhere ("Some people find these tones calming; there is no good evidence that they heal").

use egui::{Color32, RichText};
use humanity_core::i18n::{t, t_dyn};
use humanity_music::Tuning;

use crate::gui::theme::Theme;
use crate::gui::widgets::{with_hint, Hint};
use crate::gui::{widgets, GuiPage, GuiState};

/// Draw Settings > Music. Marks the settings dirty when anything changes.
pub fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: Color32) {
    let hint = state.settings.hint_display;
    let mut dirty = false;
    let tones = if state.music_dev.readout.tones.is_empty() { vec![528.0] } else { state.music_dev.readout.tones.clone() };

    // What plays: said in the Music page's own words, and the way to the one place that chooses it.
    let now = crate::gui::pages::music::now_playing_words(&state.settings.music, &state.music_dev.readout);
    let mut choose = false;
    widgets::subsection_header(ui, theme, accent, t("settings.music.what_plays"), "");
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new(now).strong().color(theme.text_primary()));
        ui.add_space(theme.spacing_xs);
        if ui.button(t("settings.music.choose")).hint("settings.music.choose_hint").clicked() {
            choose = true;
        }
        widgets::setting_hint(ui, theme, hint, t("settings.music.choose_hint"));
        ui.label(RichText::new(t("settings.music.loudness_where")).size(theme.font_size_small).color(theme.text_muted()));
    });
    if choose {
        state.go(GuiPage::Music, crate::gui::How::From);
    }

    let m = &mut state.settings.music;
    widgets::subsection_header(ui, theme, accent, t("settings.music.music"), "");
    widgets::card(ui, theme, |ui| {
        dirty |= with_hint(ui, "settings.music.music_on_hint", |ui| widgets::toggle(ui, theme, t("settings.music.music_on"), &mut m.music_on));
        widgets::setting_hint(ui, theme, hint, t("settings.music.music_on_hint"));
        dirty |= with_hint(ui, "settings.music.music_on_pages_hint", |ui| widgets::toggle(ui, theme, t("settings.music.music_on_pages"), &mut m.music_on_pages));
        widgets::setting_hint(ui, theme, hint, t("settings.music.music_on_pages_hint"));
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new(t("settings.music.tuning")).color(theme.text_secondary()));
        ui.horizontal_wrapped(|ui| {
            for tuning in Tuning::ALL {
                let selected = m.tuning == tuning;
                if ui.radio(selected, RichText::new(t_dyn(tuning.words_key())).color(theme.text_primary())).hint("settings.music.tuning_hint").clicked() && !selected {
                    m.tuning = tuning;
                    dirty = true;
                }
            }
        });
        widgets::setting_hint(ui, theme, hint, t("settings.music.tuning_hint"));
    });

    widgets::subsection_header(ui, theme, accent, t("settings.music.hum"), "");
    widgets::card(ui, theme, |ui| {
        dirty |= with_hint(ui, "settings.music.ambience_volume_hint", |ui| widgets::labeled_slider(ui, theme, t("settings.music.ambience_volume"), &mut m.ambience_volume, 0.0..=1.0));
        widgets::setting_hint(ui, theme, hint, t("settings.music.ambience_volume_hint"));
        dirty |= with_hint(ui, "settings.music.ship_hum_hint", |ui| widgets::toggle(ui, theme, t("settings.music.ship_hum"), &mut m.ship_hum));
        widgets::setting_hint(ui, theme, hint, t("settings.music.ship_hum_hint"));
        dirty |= with_hint(ui, "settings.music.ship_hum_level_hint", |ui| widgets::labeled_slider(ui, theme, t("settings.music.ship_hum_level"), &mut m.ship_hum_level, 0.0..=1.0));
        widgets::setting_hint(ui, theme, hint, t("settings.music.ship_hum_level_hint"));
    });

    widgets::subsection_header(ui, theme, accent, t("settings.music.tones"), "");
    widgets::card(ui, theme, |ui| {
        dirty |= with_hint(ui, "settings.music.tones_on_hint", |ui| widgets::toggle(ui, theme, t("settings.music.tones_on"), &mut m.tones_on));
        widgets::setting_hint(ui, theme, hint, t("settings.music.tones_on_hint"));
        if m.tones_on {
            ui.label(RichText::new(t("settings.music.tone_hz")).color(theme.text_secondary()));
            ui.horizontal_wrapped(|ui| {
                for &hz in &tones {
                    let selected = (m.tone_hz - hz).abs() < 0.01;
                    if ui.radio(selected, RichText::new(format!("{hz:.0}")).color(theme.text_primary())).hint("settings.music.tone_hz_hint").clicked() && !selected {
                        m.tone_hz = hz;
                        dirty = true;
                    }
                }
                let typed = ui.add(egui::DragValue::new(&mut m.tone_hz).range(20.0..=4000.0).speed(1.0).suffix(t("settings.music.hz_suffix"))).hint("settings.music.tone_hz_hint");
                dirty |= typed.changed();
            });
            widgets::setting_hint(ui, theme, hint, t("settings.music.tone_hz_hint"));
            dirty |= with_hint(ui, "settings.music.tone_volume_hint", |ui| widgets::labeled_slider(ui, theme, t("settings.music.tone_volume"), &mut m.tone_volume, 0.0..=1.0));
            widgets::setting_hint(ui, theme, hint, t("settings.music.tone_volume_hint"));
        }
    });
    if dirty {
        state.settings_dirty = true;
    }
}

#[cfg(test)]
mod tests {
    /// Every word Settings > Music asks for is in English (the i18n lint holds the literal keys too;
    /// this holds the tuning's, which are built from data). RED first with the 440 tuning's key
    /// missing from en.json: "settings.music.tuning_440 has no words".
    #[test]
    fn every_music_setting_has_its_words() {
        let en: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data/i18n/en.json")).unwrap()).unwrap();
        for tuning in humanity_music::Tuning::ALL {
            let key = tuning.words_key();
            let mut v = &en;
            for part in key.split('.') {
                v = &v[part];
            }
            assert!(v.as_str().is_some_and(|s| !s.is_empty()), "{key} has no words");
        }
    }
}

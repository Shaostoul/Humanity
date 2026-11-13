//! Settings > Music (docs/design/music.md, sections 7a, 10, 11 and 15): the game's live music, its
//! genre, the tuning, the ship's hum and the optional tones. Every word by key, every control with
//! its hint. The values live in `SettingsState::music` (humanity_music::MusicSettings) and are saved
//! in config.json as one `music` object.
//!
//! The genre list is the music's own data (data/music/genres/*.ron, community genres included: a
//! file dropped there is in the list), read by the engine on its music thread from the first frame,
//! music played or not, with or without an audio device (engine/music.rs; BUG-273: it was read only
//! when a sound started, so until the music had played once the list held the default alone), and
//! passed here in the Dev card's readout. With the Dev page's "turn off all reuse" switch off it is
//! read again every few seconds. Only in the first moment of a run, before that read lands, does the
//! list hold the default alone.
//!
//! The words about tuning, tones and the hum are exactly the design's (section 15): no health claim
//! anywhere ("Some people find these tones calming; there is no good evidence that they heal").
//! What the app will not do and why (CLAUDE.md, "Saying what we will not build"): some genres need
//! recorded instruments, and the app never samples copyrighted recordings, because copyright LAW
//! forbids it.
//!
//! The instrument library's section (installing CC0 sample packs, the instrument-library work) sits
//! under these, in its own file.

use egui::{Color32, RichText};
use humanity_core::i18n::{t, t_dyn};
use humanity_music::Tuning;

use crate::gui::pages::dev_music::GenreChoice;
use crate::gui::theme::Theme;
use crate::gui::widgets::{with_hint, Hint};
use crate::gui::{widgets, GuiState};

/// A genre's name for a person: its words by key, else the plain name a community file gives, else
/// its id.
pub fn genre_label(id: &str, words: &str, name: &str) -> String {
    if !words.is_empty() {
        let w = t_dyn(words);
        if w != words {
            return w.to_string();
        }
    }
    if !name.is_empty() {
        name.to_string()
    } else {
        id.to_string()
    }
}

/// Draw Settings > Music. Marks the settings dirty when anything changes.
pub fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: Color32) {
    let hint = state.settings.hint_display;
    let mut dirty = false;
    let genres = if state.music_dev.readout.genres.is_empty() {
        vec![GenreChoice { id: humanity_music::library::DEFAULT_GENRE.to_string(), words: "music.genre.mothership_calm".to_string(), about_words: "music.genre.mothership_calm_about".to_string(), ..Default::default() }]
    } else {
        state.music_dev.readout.genres.clone()
    };
    let tones = if state.music_dev.readout.tones.is_empty() { vec![528.0] } else { state.music_dev.readout.tones.clone() };
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

    widgets::subsection_header(ui, theme, accent, t("settings.music.genre"), "");
    widgets::card(ui, theme, |ui| {
        widgets::setting_hint(ui, theme, hint, t("settings.music.genre_hint"));
        ui.horizontal_wrapped(|ui| {
            for g in &genres {
                let selected = m.genre == g.id;
                if ui.radio(selected, RichText::new(genre_label(&g.id, &g.words, &g.name)).color(theme.text_primary())).hint("settings.music.genre_hint").clicked() && !selected {
                    m.genre = g.id.clone();
                    dirty = true;
                }
            }
        });
        // What the chosen genre sounds like, and which of its instruments play synthesised stand-ins
        // until a free recording of them is installed (said plainly, never silently).
        if let Some(g) = genres.iter().find(|g| g.id == m.genre) {
            let about = genre_label("", &g.about_words, &g.about);
            if !about.is_empty() {
                ui.label(RichText::new(about).color(theme.text_secondary()));
            }
            if !g.stand_ins.is_empty() {
                let parts: Vec<String> = g.stand_ins.iter().map(|k| t_dyn(k).to_string()).collect();
                ui.label(RichText::new(t("settings.music.genre_stand_ins").replace("{{parts}}", &parts.join(", "))).size(theme.font_size_small).color(theme.text_muted()));
            }
        }
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new(t("settings.music.blend_with")).color(theme.text_secondary()));
        ui.horizontal_wrapped(|ui| {
            let none = m.blend_with.is_empty();
            if ui.radio(none, RichText::new(t("settings.music.blend_none")).color(theme.text_primary())).hint("settings.music.blend_with_hint").clicked() && !none {
                m.blend_with.clear();
                dirty = true;
            }
            for g in genres.iter().filter(|g| g.id != m.genre) {
                let selected = m.blend_with == g.id;
                if ui.radio(selected, RichText::new(genre_label(&g.id, &g.words, &g.name)).color(theme.text_primary())).hint("settings.music.blend_with_hint").clicked() && !selected {
                    m.blend_with = g.id.clone();
                    dirty = true;
                }
            }
        });
        widgets::setting_hint(ui, theme, hint, t("settings.music.blend_with_hint"));
        if !m.blend_with.is_empty() {
            dirty |= with_hint(ui, "settings.music.blend_hint", |ui| widgets::labeled_slider(ui, theme, t("settings.music.blend"), &mut m.blend, 0.0..=1.0));
            widgets::setting_hint(ui, theme, hint, t("settings.music.blend_hint"));
        }
        dirty |= with_hint(ui, "settings.music.places_pick_genre_hint", |ui| widgets::toggle(ui, theme, t("settings.music.places_pick_genre"), &mut m.places_pick_genre));
        widgets::setting_hint(ui, theme, hint, t("settings.music.places_pick_genre_hint"));
        ui.label(RichText::new(t("settings.music.recordings_note")).size(theme.font_size_small).color(theme.text_muted()));
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
    /// this holds the tuning's, which are built from data) and a genre with no words shows its plain
    /// name. RED first with the 440 tuning's key missing from en.json: "settings.music.tuning_440 has
    /// no words".
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
        assert_eq!(super::genre_label("chiptune", "", "Chiptune"), "Chiptune");
        assert_eq!(super::genre_label("chiptune", "", ""), "chiptune");
    }
}

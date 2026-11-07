//! Settings > Audio > "Sound levels" (2026-11-06; docs/design/loudness.md, "Sound levels").
//!
//! The operator, told that "Even out loudness" made the button click about 9 dB louder and that it
//! is "a one-number change in the sound data": "Can we edit them in the settings menu?" So every
//! sound the game ships (each entry of `data/sounds.toml` whose file exists) is listed here,
//! grouped by its kind (the kinds of `data/audio/loudness.toml`: interface, notifications, sound
//! effects, ambience...), each with its name in plain words, a level slider from -24 dB to +12 dB
//! (0 = the sound as the app plays it now), Play to hear it at that level, and Reset; a search box
//! and Reset all above them.
//!
//! The levels are the person's, held by `humanity_core::audio::sound_levels` and saved in their
//! config (`AppConfig::sound_levels`); `data/sounds.toml` stays the shipped default and is never
//! written. The mixer applies a sound's level on top of "Even out loudness" each time the sound
//! starts (`AudioManager::one_shot`), so a slider moved here is heard the next time the sound plays,
//! and at once with Play ([`play_preview`], which the engine runs each frame).
//!
//! Every word is a key in `data/i18n/en.json` (a sound's name is `sound_names.<its id>`); every
//! control carries its plain-language hint. The website plays no game sounds, only notification
//! sounds, so its mirror (Settings > Audio & Video > Sound levels, `web/shared/sound-levels.js`)
//! lists just those.

use std::path::{Path, PathBuf};

use egui::RichText;
use humanity_core::audio::sound_levels as sl;
use humanity_core::audio::sounds::SoundCatalog;
use humanity_core::i18n::{t, t_dyn, tf};

use crate::gui::theme::Theme;
use crate::gui::widgets::{self, with_hint};
use crate::gui::HintDisplay;

/// The list of shipped sounds is read again this often while the section is open (seconds), so
/// a sound file added or a `sounds.toml` edited shows within a few seconds; with the Dev page's
/// "turn off all reuse" switch off, every frame (CLAUDE.md, the reuse rule).
const REBUILD_EVERY_S: f64 = 5.0;
/// Room kept on a row's right for its level in dB, Play and Reset, so the slider never runs under
/// them (the same trick as `widgets::labeled_slider`).
const ROW_RESERVE: f32 = 210.0;
/// The width of the level's words ("-24.0 dB"), so every row's buttons line up.
const LEVEL_WORDS_W: f32 = 64.0;

/// One sound the section lists.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct SoundRow {
    /// Its id in `data/sounds.toml` (`sfx.button_click`): the key its level is kept by.
    pub id: String,
    /// Its kind (`interface`, `notification`, `sfx`...): the group it is listed in.
    pub kind: String,
}

/// Where the section's state is kept between frames: egui's own memory, not `GuiState` (whose file
/// is at its line budget, tests/file_size_ratchet.rs; nothing outside this section and the engine's
/// half of Play reads it).
const KEPT_ID: &str = "settings_sound_levels";

/// What the section keeps between frames.
#[derive(Clone)]
pub(crate) struct SoundLevelsUi {
    /// The search box's words.
    pub search: String,
    /// The sound a Play asked for, which the engine plays and clears ([`play_preview`]).
    pub preview: Option<String>,
    /// Where the data and the sound files are: the app's `data` and `assets` beside it (a test
    /// points them at the repo's).
    pub data_dir: PathBuf,
    pub assets_dir: PathBuf,
    /// The shipped sounds and when they were read (egui's clock), rebuilt every
    /// [`REBUILD_EVERY_S`] while drawn.
    list: Option<(f64, Vec<SoundRow>)>,
}

impl Default for SoundLevelsUi {
    fn default() -> Self {
        SoundLevelsUi::reading(PathBuf::from("data"), PathBuf::from("assets"))
    }
}

impl SoundLevelsUi {
    /// The section reading the data and the sound files from these folders (the app's are `data`
    /// and `assets` beside it; a test or a snapshot points them at the repo's).
    pub(crate) fn reading(data_dir: PathBuf, assets_dir: PathBuf) -> Self {
        SoundLevelsUi { search: String::new(), preview: None, data_dir, assets_dir, list: None }
    }
}

/// Every sound the game ships: each entry of `data_dir`/sounds.toml whose file exists under
/// `assets_dir` (a catalogue entry with no file plays nothing, so it has no level to set), with its
/// kind (its own `category`, else its bus's, resolved through `data/audio/loudness.toml`), by id.
pub(crate) fn shipped_rows(data_dir: &Path, assets_dir: &Path) -> Vec<SoundRow> {
    let catalog = SoundCatalog::load(data_dir);
    let targets = humanity_core::audio::levels::LoudnessTargets::load(data_dir).ok();
    catalog
        .shipped(assets_dir)
        .into_iter()
        .map(|(id, e)| {
            let named = e.category.as_deref().unwrap_or(e.bus.as_str());
            let kind = targets.as_ref().map_or(named, |t| t.resolve(named)).to_string();
            SoundRow { id: id.clone(), kind }
        })
        .collect()
}

/// A sound's name in the person's words: `sound_names.<id>`; a sound with no words yet (a mod's)
/// shows the last part of its id with spaces, `button_click` as "button click".
pub(crate) fn sound_name(id: &str) -> String {
    let key = format!("sound_names.{id}");
    if humanity_core::i18n::has(&key) {
        return t_dyn(&key).to_string();
    }
    id.rsplit('.').next().unwrap_or(id).replace('_', " ")
}

/// A kind's name in the person's words: `sound_levels.kinds.<kind>`, else the kind's own word.
fn kind_name(kind: &str) -> String {
    let key = format!("sound_levels.kinds.{kind}");
    if humanity_core::i18n::has(&key) {
        t_dyn(&key).to_string()
    } else {
        kind.to_string()
    }
}

/// A level as the row shows it: "0.0 dB" at the default, else signed ("+3.5 dB", "-6.0 dB").
fn level_words(db: f32) -> String {
    let n = if db == 0.0 { "0.0".to_string() } else { format!("{db:+.1}") };
    tf("sound_levels.db", &[("db", &n)])
}

/// Does the row match the search words (its name, its kind's name or its id, ignoring case)?
fn matches(row: &SoundRow, words: &str) -> bool {
    let words = words.trim().to_lowercase();
    words.is_empty() || sound_name(&row.id).to_lowercase().contains(&words) || kind_name(&row.kind).to_lowercase().contains(&words) || row.id.to_lowercase().contains(&words)
}

/// Draw Settings > Audio > Sound levels with the state kept in egui's memory (`KEPT_ID`), as the
/// Settings page does. True when a level changed (the caller saves the settings).
pub(crate) fn draw_kept(ui: &mut egui::Ui, theme: &Theme, hint: HintDisplay, accent: egui::Color32) -> bool {
    let id = egui::Id::new(KEPT_ID);
    let mut s = ui.ctx().data(|d| d.get_temp::<SoundLevelsUi>(id)).unwrap_or_default();
    let dirty = draw(ui, theme, &mut s, hint, accent);
    ui.ctx().data_mut(|d| d.insert_temp(id, s));
    dirty
}

/// Draw Settings > Audio > Sound levels from `s`. True when a level changed.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, s: &mut SoundLevelsUi, hint: HintDisplay, accent: egui::Color32) -> bool {
    let now = ui.ctx().input(|i| i.time);
    let stale = s.list.as_ref().is_none_or(|(at, _)| now - at >= REBUILD_EVERY_S || now < *at);
    if stale || !humanity_core::reuse::enabled() {
        s.list = Some((now, shipped_rows(&s.data_dir, &s.assets_dir)));
    }
    let rows = s.list.as_ref().map(|(_, r)| r.clone()).unwrap_or_default();
    let mut dirty = false;
    widgets::subsection_header(ui, theme, accent, t("sound_levels.title"), "");
    widgets::card(ui, theme, |ui| {
        // "dB" is marked for its glossary card (data/glossary.json, "decibel").
        widgets::glossary_note(ui, theme, "sound_levels.note");
        ui.add_space(theme.spacing_sm);
        ui.horizontal(|ui| {
            with_hint(ui, "sound_levels.search_hint", |ui| widgets::search_bar(ui, theme, &mut s.search, t("sound_levels.search_placeholder")));
            if widgets::Button::secondary(t("sound_levels.reset_all")).hint("sound_levels.reset_all_hint").disabled(!sl::any()).show(ui, theme) && sl::reset_all() {
                dirty = true;
            }
        });
        if rows.is_empty() {
            ui.label(RichText::new(t("sound_levels.none_shipped")).color(theme.text_muted()));
            return;
        }
        // The kinds that have a sound matching the search, by their names in the person's words;
        // each kind's sounds by name.
        let shown: Vec<&SoundRow> = rows.iter().filter(|r| matches(r, &s.search)).collect();
        if shown.is_empty() {
            ui.label(RichText::new(t("sound_levels.none_found")).color(theme.text_muted()));
            return;
        }
        let mut kinds: Vec<(String, &str)> = Vec::new();
        for r in &shown {
            if !kinds.iter().any(|(_, k)| *k == r.kind) {
                kinds.push((kind_name(&r.kind), r.kind.as_str()));
            }
        }
        kinds.sort();
        for (kind_words, kind) in kinds {
            ui.add_space(theme.spacing_sm);
            ui.label(RichText::new(kind_words).color(theme.text_secondary()).strong());
            let mut group: Vec<(String, &SoundRow)> = shown.iter().filter(|r| r.kind == kind).map(|r| (sound_name(&r.id), *r)).collect();
            group.sort();
            for (name, row) in group {
                dirty |= draw_row(ui, theme, s, &name, row);
            }
        }
        ui.add_space(theme.spacing_xs);
        widgets::setting_hint(ui, theme, hint, t("sound_levels.level_hint"));
    });
    dirty
}

/// One sound's row: its name, the level slider, the level in dB, Play and Reset. True when its
/// level changed.
fn draw_row(ui: &mut egui::Ui, theme: &Theme, s: &mut SoundLevelsUi, name: &str, row: &SoundRow) -> bool {
    let level = sl::level_db(&row.id);
    let mut db = level;
    let mut changed = false;
    ui.push_id(("sound_level", row.id.as_str()), |ui| {
        widgets::settings_row(ui, theme, name, |ui| {
            if with_hint(ui, "sound_levels.level_hint", |ui| widgets::custom_slider_capped(ui, theme, &mut db, sl::MIN_DB..=sl::MAX_DB, ROW_RESERVE)) {
                changed |= sl::set_level(&row.id, db);
            }
            let words = level_words(sl::level_db(&row.id));
            ui.add_sized(egui::vec2(LEVEL_WORDS_W, ui.spacing().interact_size.y), egui::Label::new(RichText::new(words).color(theme.text_muted()).size(theme.font_size_small)));
            if widgets::Button::secondary(t("sound_levels.play")).hint("sound_levels.play_hint").show(ui, theme) {
                s.preview = Some(row.id.clone());
            }
            if widgets::Button::ghost(t("sound_levels.reset")).hint("sound_levels.reset_hint").disabled(level == 0.0).show(ui, theme) {
                changed |= sl::reset(&row.id);
            }
        });
    });
    changed
}

/// Per frame (the engine's audio stage): play the sound the Settings page's Play asked for (kept in
/// `ctx`'s memory by [`draw_kept`]) and clear the ask. The amplitude it played at, for the tests.
pub(crate) fn play_preview<B: kira::backend::Backend>(ctx: &egui::Context, audio: &mut humanity_core::audio::AudioManager<B>, catalog: &SoundCatalog) -> Option<f64> {
    // Taken in place: nothing is copied on the frames (nearly all) with no Play asked for.
    let (id, assets_dir) = ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_default::<SoundLevelsUi>(egui::Id::new(KEPT_ID));
        s.preview.take().map(|id| (id, s.assets_dir.clone()))
    })?;
    play(&id, &assets_dir, audio, catalog)
}

/// Play the sound `id` the way the game plays it (its authored volume and bus from the catalogue,
/// its loudness level, and the person's level for it as it is now), stopping the last preview. The
/// amplitude it played at, for the tests.
fn play<B: kira::backend::Backend>(id: &str, assets_dir: &Path, audio: &mut humanity_core::audio::AudioManager<B>, catalog: &SoundCatalog) -> Option<f64> {
    let entry = catalog.get(id)?;
    // Spelled as the game spells a sound's path (`assets/<path>`), so both share one decoded copy.
    let path = format!("{}/{}", assets_dir.display(), entry.path);
    match audio.preview(&path, entry.volume as f64, &entry.bus) {
        Ok(amplitude) => Some(amplitude),
        Err(e) => {
            log::warn!("[Audio] Sound levels, Play {id}: {e}");
            None
        }
    }
}

#[cfg(test)]
#[path = "sound_levels_tests.rs"]
mod tests;

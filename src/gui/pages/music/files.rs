//! MUSIC FILES IN AND OUT (docs/design/music-page.md sections 12 and 15a, rung 2): the piece playing
//! saved as MEI (the open notation format the written pieces use) or as a Standard MIDI File, to a
//! name and a folder the person chooses, so MuseScore, Verovio and other music software open it; and a
//! MIDI file opened into the Written music catalogue as a piece of the person's own (humanity_music::own:
//! kept in their data folder, its details theirs to change, refused in words when it cannot be read).
//!
//! A written piece saves as MEI by copying its file exactly (its lyrics, dynamics and engraving
//! intact) and as MIDI from its notes; a composed piece is written down by the composer run dry for its
//! set length ("A new piece" minutes, else the length chosen for audio exports) and saved from that
//! sheet, so the file holds the notes the piano roll drew.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{Color32, RichText};
use humanity_core::i18n::t;
use humanity_music::scores::{ScoreEntry, WrittenPiece};
use humanity_music::sheet::Sheet;
use humanity_music::smf::{write_smf, Notated};

use super::{export_dir, role_words, score_words};
use crate::gui::theme::Theme;
use crate::gui::widgets::file_browser::{file_picker_modal, FilePickerResult, FilePickerState};
use crate::gui::widgets::{self, Button, ButtonSize, Hint};
use crate::gui::GuiState;

/// The two music-file formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Mei,
    Midi,
}

impl Format {
    pub fn ext(self) -> &'static str {
        match self {
            Format::Mei => "mei",
            Format::Midi => "mid",
        }
    }
}

/// The largest MIDI file the page opens (the importer's own limit).
const IMPORT_MOST: u64 = humanity_music::smf::MAX_BYTES as u64;

/// A piece of the person's own whose details are being changed: its id and each box's text.
#[derive(Clone, Debug, PartialEq)]
pub struct OwnEdit {
    pub id: String,
    pub name: String,
    pub composer: String,
    pub year: String,
    pub source: String,
    pub licence: String,
    /// Remove was clicked once (the second click removes it).
    pub removing: bool,
}

/// A name made safe for every file system (letters, digits, spaces, dashes and underscores).
pub fn safe_name(name: &str) -> String {
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' }).collect();
    let safe = safe.trim().to_string();
    if safe.is_empty() {
        "HumanityOS music".into()
    } else {
        safe
    }
}

/// A written piece as a file: MEI, its own file copied exactly; MIDI, its notes.
pub fn written_bytes(piece: &WrittenPiece, format: Format) -> Result<Vec<u8>, String> {
    match format {
        Format::Mei => {
            let path = match &piece.entry.folder {
                Some(dir) => dir.join(&piece.entry.file),
                None => humanity_music::scores::index_path(&humanity_core::storage::data_dir()).with_file_name(&piece.entry.file),
            };
            std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))
        }
        Format::Midi => Ok(write_smf(&Notated::from_mei(&piece.score, &piece.entry))),
    }
}

/// A composed piece (its sheet, a person's name for it) as a file.
pub fn composed_bytes(sheet: &Sheet, title: &str, format: Format) -> Vec<u8> {
    let names: Vec<String> = (0..humanity_music::tune::ROLES).map(|k| role_words(k).to_string()).collect();
    let n = Notated::from_sheet(sheet, &names, title);
    match format {
        Format::Midi => write_smf(&n),
        Format::Mei => humanity_music::mei_write::write_mei(&n, &t("music_page.files_availability")).into_bytes(),
    }
}

/// Write `bytes` as `<name>.<ext>` in `dir` (made if missing). The path written.
pub fn write_file(dir: &Path, name: &str, format: Format, bytes: &[u8]) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join(format!("{}.{}", safe_name(name), format.ext()));
    std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

/// The piece playing as a file, with a name for it: a written piece, or a composed piece written down
/// for its set length (or `minutes` when it has none).
fn playing_bytes(state: &GuiState, format: Format) -> Result<Vec<u8>, String> {
    let r = &state.music_dev.readout;
    if let Some(piece) = r.written_piece.as_ref() {
        return written_bytes(piece, format);
    }
    let p = r.playing.as_ref().ok_or_else(|| t("music_page.files_nothing").to_string())?;
    let now = r.score.map(|s| s.seconds).unwrap_or(0.0);
    let seconds = match r.next_piece_s {
        Some(rem) => now + rem as f64,
        None => state.music_page.export_minutes as f64 * 60.0,
    }
    .clamp(10.0, 60.0 * 60.0);
    let sheet = humanity_music::sheet::transcribe(&p.res, p.seed, &p.ctx, 48000.0, seconds);
    Ok(composed_bytes(&sheet, &playing_name(state), format))
}

/// A name for the piece playing: a written piece's title, else its genre's words and its seed.
pub fn playing_name(state: &GuiState) -> String {
    let r = &state.music_dev.readout;
    if let Some(piece) = r.written_piece.as_ref() {
        return score_words(&piece.entry).0;
    }
    let sh = super::shown(&state.settings.music, r);
    super::like::liked_favourite(state, &sh).name
}

/// Your music's "Music files" card: the piece playing saved as MIDI or MEI to a name and folder the
/// person chooses, and a MIDI file opened as a piece of their own.
pub(super) fn files_card(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: Color32) {
    let hint = state.settings.hint_display;
    widgets::subsection_header(ui, theme, accent, t("music_page.files_title"), t("music_page.files_note"));
    widgets::card(ui, theme, |ui| {
        let playing = state.music_dev.readout.written_piece.is_some() || state.music_dev.readout.playing.is_some();
        if state.music_page.file_name.is_empty() && playing {
            state.music_page.file_name = playing_name(state);
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(t("music_page.files_name")).color(theme.text_secondary()));
            ui.add(egui::TextEdit::singleline(&mut state.music_page.file_name).desired_width(320.0)).hint("music_page.files_name_hint");
            if playing && Button::ghost(t("music_page.files_name_playing")).size(ButtonSize::Small).hint("music_page.files_name_playing_hint").show(ui, theme) {
                state.music_page.file_name = playing_name(state);
            }
        });
        let dir = state.music_page.file_dir.clone().or_else(|| state.music_page.export_dir_override.clone()).unwrap_or_else(export_dir);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(t("music_page.files_folder").replace("{{dir}}", &dir.display().to_string())).size(theme.font_size_small).color(theme.text_secondary()));
            if Button::new(t("music_page.files_choose_folder")).size(ButtonSize::Small).hint("music_page.files_choose_folder_hint").show(ui, theme) {
                state.music_page.export_picker = Some(FilePickerState::new_dir_picker(Some(dir.clone())));
            }
        });
        ui.horizontal_wrapped(|ui| {
            for (format, words, h) in [(Format::Midi, "music_page.files_save_midi", "music_page.files_save_midi_hint"), (Format::Mei, "music_page.files_save_mei", "music_page.files_save_mei_hint")] {
                if Button::primary(t(words)).size(ButtonSize::Small).disabled(!playing).hint(h).show(ui, theme) {
                    let name = if state.music_page.file_name.trim().is_empty() { playing_name(state) } else { state.music_page.file_name.clone() };
                    state.music_page.file_msg = Some(match playing_bytes(state, format).and_then(|b| write_file(&dir, &name, format, &b)) {
                        Ok(path) => t("music_page.files_saved").replace("{{path}}", &path.display().to_string()),
                        Err(e) => t("music_page.files_failed").replace("{{why}}", &e),
                    });
                }
            }
        });
        if !playing {
            ui.label(RichText::new(t("music_page.files_nothing")).size(theme.font_size_small).color(theme.text_muted()));
        }
        if let Some(m) = &state.music_page.file_msg {
            ui.label(RichText::new(m).size(theme.font_size_small).color(accent));
        }
        widgets::setting_hint(ui, theme, hint, t("music_page.files_hint"));
        ui.add_space(theme.spacing_sm);
        import_button(ui, theme, state);
    });
}

/// "Open a MIDI file": the picker, then the import (its result said beside it).
pub(super) fn import_button(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.horizontal_wrapped(|ui| {
        if Button::new(t("music_page.import_midi")).size(ButtonSize::Small).hint("music_page.import_midi_hint").show(ui, theme) {
            state.music_page.import_picker = Some(FilePickerState::new(&["mid", "midi"], IMPORT_MOST).with_pick_verb(t("music_page.import_pick")));
        }
        if let Some(m) = &state.music_page.own_msg {
            ui.label(RichText::new(m).size(theme.font_size_small).color(theme.text_secondary()));
        }
    });
}

/// Import the MIDI file at `path` as a piece of the person's own: kept in their folder and listed,
/// chosen in the catalogue. The words the page shows (what was imported, or why not).
pub fn import_file(state: &mut GuiState, path: &Path) -> String {
    let Some(dir) = state.music_page.own_dir.clone() else { return t("music_page.import_no_folder").to_string() };
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("piece.mid").to_string();
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return t("music_page.import_refused").replace("{{why}}", &e.to_string()),
    };
    let mut taken: Vec<String> = state.music_page.own.iter().map(|e| e.id.clone()).collect();
    if let Some(l) = state.music_dev.readout.lib.as_deref() {
        taken.extend(l.scores.iter().map(|e| e.id.clone()));
    }
    let imported = match humanity_music::own::import_midi(&bytes, &name, &taken) {
        Ok(i) => i,
        Err(why) => return t("music_page.import_refused").replace("{{why}}", &why),
    };
    let (notes, parts) = (imported.notes, imported.parts);
    match humanity_music::own::keep(&dir, &mut state.music_page.own, imported) {
        Ok(entry) => {
            state.music_page.written_selected = Some(entry.id.clone());
            t("music_page.import_done").replace("{{title}}", &entry.name).replace("{{notes}}", &notes.to_string()).replace("{{parts}}", &parts.to_string())
        }
        Err(e) => t("music_page.import_refused").replace("{{why}}", &e),
    }
}

/// The open pickers (an import's file, an export's folder), drawn over the page.
pub(super) fn pickers(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    if let Some(mut picker) = state.music_page.import_picker.take() {
        match file_picker_modal(ctx, theme, &mut picker, t("music_page.import_picker_title")) {
            FilePickerResult::Open => state.music_page.import_picker = Some(picker),
            FilePickerResult::Cancelled => {}
            FilePickerResult::Picked(paths) => {
                if let Some(p) = paths.first() {
                    let msg = import_file(state, p);
                    state.music_page.own_msg = Some(msg);
                    state.music_page.tab = super::Tab::Written;
                }
            }
        }
    }
    if let Some(mut picker) = state.music_page.export_picker.take() {
        match file_picker_modal(ctx, theme, &mut picker, t("music_page.files_picker_title")) {
            FilePickerResult::Open => state.music_page.export_picker = Some(picker),
            FilePickerResult::Cancelled => {}
            FilePickerResult::Picked(paths) => state.music_page.file_dir = paths.first().cloned(),
        }
    }
}

/// A piece of the person's own in the details panel: Edit details (title, composer, year, where it
/// came from, its licence) and Remove (twice, since it deletes their file).
pub(super) fn own_details(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, entry: &ScoreEntry) {
    ui.label(RichText::new(t("music_page.own_note")).size(theme.font_size_small).color(theme.text_muted()));
    let editing = state.music_page.editing.as_ref().is_some_and(|e| e.id == entry.id);
    if !editing {
        ui.horizontal_wrapped(|ui| {
            if Button::new(t("music_page.own_edit")).hint("music_page.own_edit_hint").show(ui, theme) {
                state.music_page.editing = Some(OwnEdit {
                    id: entry.id.clone(),
                    name: entry.name.clone(),
                    composer: entry.composer.clone(),
                    year: if entry.year > 0 { entry.year.to_string() } else { String::new() },
                    source: entry.source.clone(),
                    licence: entry.licence.clone(),
                    removing: false,
                });
            }
        });
        return;
    }
    let Some(mut e) = state.music_page.editing.take() else { return };
    let mut done = false;
    let mut save = false;
    let mut remove = false;
    egui::Grid::new("music_page_own_edit").num_columns(2).spacing([theme.spacing_sm, theme.spacing_xs]).show(ui, |ui| {
        for (words, h, text) in [
            ("music_page.own_title", "music_page.own_title_hint", &mut e.name),
            ("music_page.own_composer", "music_page.own_composer_hint", &mut e.composer),
            ("music_page.own_year", "music_page.own_year_hint", &mut e.year),
            ("music_page.own_source", "music_page.own_source_hint", &mut e.source),
            ("music_page.own_licence", "music_page.own_licence_hint", &mut e.licence),
        ] {
            ui.label(RichText::new(t(words)).color(theme.text_secondary()));
            ui.add(egui::TextEdit::singleline(text).desired_width(300.0)).hint(h);
            ui.end_row();
        }
    });
    ui.horizontal_wrapped(|ui| {
        if Button::primary(t("music_page.own_save")).hint("music_page.own_save_hint").show(ui, theme) {
            save = true;
        }
        if Button::new(t("music_page.own_cancel")).hint("music_page.own_cancel_hint").show(ui, theme) {
            done = true;
        }
        let (words, h) = if e.removing { ("music_page.own_remove_sure", "music_page.own_remove_sure_hint") } else { ("music_page.own_remove", "music_page.own_remove_hint") };
        if Button::danger(t(words)).hint(h).show(ui, theme) {
            if e.removing {
                remove = true;
            } else {
                e.removing = true;
            }
        }
    });
    let Some(dir) = state.music_page.own_dir.clone() else { return };
    if save {
        if let Some(x) = state.music_page.own.iter_mut().find(|x| x.id == e.id) {
            x.name = e.name.trim().to_string();
            x.composer = e.composer.trim().to_string();
            x.year = e.year.trim().parse().unwrap_or(0);
            x.source = e.source.trim().to_string();
            x.licence = e.licence.trim().to_string();
        }
        state.music_page.own_msg = Some(match humanity_music::own::save(&dir, &state.music_page.own) {
            Ok(()) => t("music_page.own_saved").to_string(),
            Err(why) => t("music_page.files_failed").replace("{{why}}", &why),
        });
        done = true;
    }
    if remove {
        state.music_page.own_msg = Some(match humanity_music::own::remove(&dir, &mut state.music_page.own, &e.id) {
            Ok(()) => t("music_page.own_removed").replace("{{title}}", &e.name),
            Err(why) => t("music_page.files_failed").replace("{{why}}", &why),
        });
        if state.settings.music.written == e.id {
            state.settings.music.written.clear();
            state.settings_dirty = true;
        }
        state.music_page.written_selected = None;
        done = true;
    }
    if !done {
        state.music_page.editing = Some(e);
    }
}

/// Every written piece the catalogue lists: the shipped ones, then the person's own.
pub fn all_scores(state: &GuiState) -> Vec<ScoreEntry> {
    let mut v: Vec<ScoreEntry> = state.music_dev.readout.lib.as_deref().map(|l| l.scores.clone()).unwrap_or_default();
    v.extend(state.music_page.own.iter().cloned());
    v
}

/// A written piece's entry by id, shipped or the person's own.
pub fn entry_of(state: &GuiState, id: &str) -> Option<ScoreEntry> {
    state.music_dev.readout.lib.as_deref().and_then(|l| l.score(id)).or_else(|| state.music_page.own.iter().find(|e| e.id == id)).cloned()
}

/// Save a written piece (not necessarily the one playing) as a file, named by its title, in the folder
/// chosen for music files. The words the page shows.
pub(super) fn save_written(state: &GuiState, e: &ScoreEntry, format: Format) -> String {
    let dir = state.music_page.file_dir.clone().or_else(|| state.music_page.export_dir_override.clone()).unwrap_or_else(export_dir);
    let piece = match state.music_dev.readout.written_piece.as_ref().filter(|p| p.entry.id == e.id) {
        Some(p) => Ok(p.clone()),
        None => humanity_music::scores::load_piece(&humanity_core::storage::data_dir(), e).map(Arc::new),
    };
    match piece.and_then(|p| written_bytes(&p, format)).and_then(|b| write_file(&dir, &score_words(e).0, format, &b)) {
        Ok(path) => t("music_page.files_saved").replace("{{path}}", &path.display().to_string()),
        Err(why) => t("music_page.files_failed").replace("{{why}}", &why),
    }
}

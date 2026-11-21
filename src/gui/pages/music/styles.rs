//! The Music page's SOUND STYLES (docs/design/music-page.md section 15a, rung 3a): the style written
//! music plays in, chosen for all written music (Written music, and a card each on Explore) or for one
//! piece (its details), kept in the person's settings (`MusicSettings::style`, `styles`); and how the
//! chosen style plays a piece, staff by staff, with every program or drum that has no instrument of
//! ours said in words (crate::style's `StyleNote`), never dropped in silence.
//!
//! The styles are data (data/music/styles/*.ron, read with the music's library): a new style file is
//! a new choice and a new card here with no code change. Every word by key, every control with a hint
//! that says what changes.

use egui::RichText;
use humanity_core::i18n::{t, t_dyn};
use humanity_music::scores::ScoreEntry;
use humanity_music::settings::AS_WRITTEN;
use humanity_music::style::{is_imported, SoundStyle, StyleNote, IMPORTED_STYLE};

use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Hint};
use crate::gui::GuiState;

/// The styles the music's data offers (none before it is read).
pub fn styles_of(state: &GuiState) -> Vec<SoundStyle> {
    state.music_dev.readout.lib.as_deref().map(|l| l.styles.clone()).unwrap_or_default()
}

/// A style's name for a person: its words by key, else a community file's plain name, else its id.
pub fn style_name(s: &SoundStyle) -> String {
    super::genre_label(&s.id, &s.words, &s.name)
}

/// A style's one plain sentence: what changes when it is chosen.
pub fn style_plain(s: &SoundStyle) -> String {
    super::genre_label("", &s.plain_words, &s.plain)
}

/// The name of style `id` among `styles` (its id made readable when no file names it).
fn name_of(styles: &[SoundStyle], id: &str) -> String {
    styles.iter().find(|s| s.id == id).map(style_name).unwrap_or_else(|| id.replace('_', " "))
}

/// A General MIDI program's name, numbered as people usually count them (1 to 128).
pub fn program_words(p: u8) -> String {
    t("music_page.style_program").replace("{{n}}", &(p as u32 + 1).to_string()).replace("{{name}}", t_dyn(&format!("music.gm.program_{p}")))
}

/// A General MIDI drum key's name (its number when the drum map names it nothing we know).
pub fn drum_words(k: u8) -> String {
    let key = format!("music.gm.drum_{k}");
    let w = t_dyn(&key);
    if w != key {
        w.to_string()
    } else {
        t("music_page.style_drum_key").replace("{{n}}", &k.to_string())
    }
}

/// A staff's name: the file's own (the index keeps none, so its number and role).
fn staff_words(e: &ScoreEntry, staff: u16) -> String {
    let role = e.staves.iter().find(|s| s.staff == staff).and_then(|s| humanity_music::tune::ROLE_IDS.iter().position(|r| *r == s.role)).map(super::role_words).unwrap_or_default();
    t("music_page.style_staff").replace("{{n}}", &staff.to_string()).replace("{{role}}", &role)
}

/// One note in words: what has no instrument of ours, and what plays it instead.
pub fn note_words(e: &ScoreEntry, n: &StyleNote) -> String {
    match n {
        StyleNote::MissingProgram { staff, program, plays_on } => t("music_page.style_missing_program").replace("{{staff}}", &staff_words(e, *staff)).replace("{{program}}", &program_words(*program)).replace("{{plays_on}}", &super::voice_words(plays_on)),
        StyleNote::MissingDrum { staff, key, plays_on } => t("music_page.style_missing_drum").replace("{{staff}}", &staff_words(e, *staff)).replace("{{drum}}", &drum_words(*key)).replace("{{plays_on}}", &super::voice_words(plays_on)),
        StyleNote::LeftOut { staff } => t("music_page.style_left_out").replace("{{staff}}", &staff_words(e, *staff)),
    }
}

/// The style piece `e` plays in now (None: its own instruments).
pub fn style_now(state: &GuiState, e: &ScoreEntry) -> Option<String> {
    state.settings.music.style_for(&e.id, is_imported(e))
}

/// What the person is told about piece `e` in style `style`: the engine's notes while it plays in
/// that style (each drum key it strikes included), else what the style decides from the index alone.
pub fn notes_for(state: &GuiState, styles: &[SoundStyle], e: &ScoreEntry, style: Option<&str>) -> Vec<StyleNote> {
    let r = &state.music_dev.readout;
    if r.written.as_deref() == Some(e.id.as_str()) && r.written_style.as_deref() == style {
        return r.style_notes.clone();
    }
    match humanity_music::style::find(styles, style) {
        Some(s) => s.plan(e, &[]).1,
        None => Vec::new(),
    }
}

/// Written music's own choice: the style every written piece plays in unless it has its own. True
/// when the settings changed.
pub fn all_written(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) -> bool {
    let hint = state.settings.hint_display;
    let styles = styles_of(state);
    let m = &mut state.settings.music;
    let before = m.style.clone();
    let shown = if m.style.is_empty() { t("music_page.style_as_written").to_string() } else { name_of(&styles, &m.style) };
    // The name above its list, the list no wider than the page, a long choice cut short with an
    // ellipsis (a phone's 412 points hold it).
    ui.label(RichText::new(t("music_page.style_all")).color(theme.text_secondary()));
    let w = (ui.available_width() - 2.0 * theme.spacing_md).clamp(120.0, 420.0);
    {
        egui::ComboBox::from_id_salt("music_page_style_all")
            .width(w)
            .truncate()
            .selected_text(shown)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut m.style, String::new(), t("music_page.style_as_written")).hint("music_page.style_as_written_hint");
                for s in &styles {
                    ui.selectable_value(&mut m.style, s.id.clone(), style_name(s)).hint("music_page.style_choice_hint");
                }
            })
            .response
            .hint("music_page.style_all_hint");
    }
    widgets::setting_hint(ui, theme, hint, t("music_page.style_all_hint"));
    m.style != before
}

/// A piece's own choice and how its style plays it: each staff and what plays it, and every note in
/// words. True when the settings changed.
pub fn for_piece(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, e: &ScoreEntry) -> bool {
    let hint = state.settings.hint_display;
    let styles = styles_of(state);
    let imported = is_imported(e);
    let m = &mut state.settings.music;
    let own = m.styles.get(&e.id).cloned().unwrap_or_default();
    let all_words = match (m.style.as_str(), imported) {
        ("", true) => name_of(&styles, IMPORTED_STYLE),
        ("", false) => t("music_page.style_as_written").to_string(),
        (id, _) => name_of(&styles, id),
    };
    let same = t("music_page.style_same").replace("{{style}}", &all_words);
    let shown = match own.as_str() {
        "" => same.clone(),
        AS_WRITTEN => t("music_page.style_as_written").to_string(),
        id => name_of(&styles, id),
    };
    let mut pick = own.clone();
    ui.label(RichText::new(t("music_page.style_piece")).color(theme.text_secondary()));
    let w = (ui.available_width() - 2.0 * theme.spacing_md).clamp(120.0, 420.0);
    {
        egui::ComboBox::from_id_salt(("music_page_style_piece", e.id.as_str()))
            .width(w)
            .truncate()
            .selected_text(shown)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut pick, String::new(), &same).hint("music_page.style_same_hint");
                ui.selectable_value(&mut pick, AS_WRITTEN.to_string(), t("music_page.style_as_written")).hint("music_page.style_as_written_hint");
                for s in &styles {
                    ui.selectable_value(&mut pick, s.id.clone(), style_name(s)).hint("music_page.style_choice_hint");
                }
            })
            .response
            .hint("music_page.style_piece_hint");
    }
    widgets::setting_hint(ui, theme, hint, t("music_page.style_piece_hint"));
    let changed = pick != own;
    if changed {
        if pick.is_empty() {
            m.styles.remove(&e.id);
        } else {
            m.styles.insert(e.id.clone(), pick);
        }
    }
    // How it plays: the style's sentence, each staff's instrument, and what has none of ours.
    let now = state.settings.music.style_for(&e.id, imported);
    if let Some(s) = humanity_music::style::find(&styles, now.as_deref()) {
        ui.label(RichText::new(style_plain(s)).size(theme.font_size_small).color(theme.text_secondary()));
        let (plans, _) = s.plan(e, &[]);
        for p in plans.iter().filter(|p| !p.left_out) {
            let plays = match &p.voice {
                Some(v) => super::voice_words(v.part.as_deref().unwrap_or(&v.instrument)),
                None => {
                    let st = e.staves.iter().find(|x| x.staff == p.staff);
                    if st.is_some_and(|x| x.drums) {
                        t("music_page.style_drum_map").to_string()
                    } else {
                        st.map(|x| super::voice_words(x.part.as_deref().unwrap_or(&x.instrument))).unwrap_or_default()
                    }
                }
            };
            let who = match p.program {
                Some(pr) => format!("{} ({})", staff_words(e, p.staff), program_words(pr)),
                None => staff_words(e, p.staff),
            };
            widgets::detail_row(ui, theme, &who, &plays);
        }
    }
    for n in notes_for(state, &styles, e, now.as_deref()) {
        ui.label(RichText::new(note_words(e, &n)).size(theme.font_size_small).color(theme.warning()));
    }
    changed
}

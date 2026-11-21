//! WRITTEN MUSIC AS A CATALOGUE (docs/design/music-page.md section 10; the operator, 2026-11-20: "the
//! written music we want to make like a searchable list for infinite songs, including the little
//! details, like the origin/source"). Every written piece in `data/music/scores/index.ron` is a row a
//! person can search (any word: title, composer, edition, source, typesetter, instrument), sort
//! (title, composer, era, year, length) and filter (composer, era); choosing a row opens its details
//! beside the list (under it on a narrow page): who wrote it and when, the key, the length, the
//! instruments, the edition, the source with a link, the typesetter, the licence, the credit and the
//! basis of its tempo. Adding a piece is adding its file and its index entry (infinite-of-X).
//!
//! Play keeps the piece's id in the person's settings (`MusicSettings::written`, score-player.md
//! section 10), so it plays everywhere, page open or not, until Stop clears it; the game's one score
//! takes it over at its next bar line.

use egui::{Color32, RichText};
use humanity_core::i18n::{t, t_dyn};
use humanity_music::scores::ScoreEntry;

use super::{score_words, voice_words};
use crate::gui::theme::Theme;
use crate::gui::widgets::Hint;
use crate::gui::{widgets, GuiState};

/// What the list is sorted by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WrittenSort {
    #[default]
    Title,
    Composer,
    Era,
    Year,
    Length,
}

impl WrittenSort {
    pub const ALL: [WrittenSort; 5] = [WrittenSort::Title, WrittenSort::Composer, WrittenSort::Era, WrittenSort::Year, WrittenSort::Length];
    /// Its words key.
    pub fn key(self) -> &'static str {
        match self {
            WrittenSort::Title => "music_page.sort_title",
            WrittenSort::Composer => "music_page.sort_composer",
            WrittenSort::Era => "music_page.sort_era",
            WrittenSort::Year => "music_page.sort_year",
            WrittenSort::Length => "music_page.sort_length",
        }
    }
}

/// The list's search text, its sort, and its filters (a composer's and an era's words as the page
/// shows them; empty: every one).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WrittenQuery {
    pub text: String,
    pub sort: WrittenSort,
    pub composer: String,
    pub era: String,
}

/// A piece's era in words (its key's, else a community file's plain words).
pub fn era_words(e: &ScoreEntry) -> String {
    if !e.era_words.is_empty() {
        let w = t_dyn(&e.era_words);
        if w != e.era_words {
            return w.to_string();
        }
    }
    e.era.clone()
}

/// The instruments a piece is written for, by their parts' words (each staff's part, else its
/// instrument), each once.
pub fn instruments(e: &ScoreEntry) -> String {
    let mut named: Vec<String> = Vec::new();
    for s in &e.staves {
        let w = match &s.part {
            Some(p) => voice_words(p),
            None => {
                let key = format!("music.instrument.{}", s.instrument);
                let w = t_dyn(&key);
                if w != key {
                    w.to_string()
                } else {
                    s.instrument.replace('_', " ")
                }
            }
        };
        if !named.contains(&w) {
            named.push(w);
        }
    }
    named.join(", ")
}

/// A length in minutes and seconds ("2:36").
pub fn length_words(s: f32) -> String {
    let s = s.max(0.0).round() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The rows of `entries` the query keeps, in its order (indices into `entries`). The search matches
/// every word typed against the piece's title, composer, era, year, edition, source, typesetter,
/// licence and instruments, case aside.
pub fn catalogue(entries: &[ScoreEntry], q: &WrittenQuery) -> Vec<usize> {
    let words: Vec<String> = q.text.split_whitespace().map(|w| w.to_lowercase()).collect();
    let mut keep: Vec<(usize, String, String, String)> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let (title, composer) = score_words(e);
        let era = era_words(e);
        if (!q.composer.is_empty() && composer != q.composer) || (!q.era.is_empty() && era != q.era) {
            continue;
        }
        if !words.is_empty() {
            let hay = [&title, &composer, &era, &e.year.to_string(), &e.edition, &e.source, &e.typesetter, &e.licence, &instruments(e)].map(|s| s.to_lowercase()).join(" ");
            if !words.iter().all(|w| hay.contains(w.as_str())) {
                continue;
            }
        }
        // A composer sorts by the last word of the name (the surname, as a catalogue lists them).
        let surname = composer.split_whitespace().last().unwrap_or("").to_lowercase();
        keep.push((i, title.to_lowercase(), format!("{surname} {}", composer.to_lowercase()), era));
    }
    // Eras in the order they came (each era's earliest year among the pieces), not by their names.
    let era_from = |era: &str| keep.iter().filter(|k| k.3 == era).map(|k| entries[k.0].year).filter(|&y| y > 0).min().unwrap_or(u32::MAX);
    let firsts: Vec<u32> = keep.iter().map(|k| era_from(&k.3)).collect();
    let mut keep: Vec<_> = keep.into_iter().zip(firsts).collect();
    keep.sort_by(|(a, fa), (b, fb)| {
        let (ea, eb) = (&entries[a.0], &entries[b.0]);
        let by = match q.sort {
            WrittenSort::Title => std::cmp::Ordering::Equal,
            WrittenSort::Composer => a.2.cmp(&b.2),
            WrittenSort::Era => fa.cmp(fb).then_with(|| a.3.cmp(&b.3)).then(ea.year.cmp(&eb.year)),
            WrittenSort::Year => ea.year.cmp(&eb.year),
            WrittenSort::Length => ea.length_s.total_cmp(&eb.length_s),
        };
        by.then_with(|| a.1.cmp(&b.1))
    });
    keep.into_iter().map(|(k, _)| k.0).collect()
}

/// The words a filter list offers: each distinct value, sorted.
fn choices(entries: &[ScoreEntry], of: impl Fn(&ScoreEntry) -> String) -> Vec<String> {
    let mut v: Vec<String> = entries.iter().map(of).filter(|s| !s.is_empty()).collect();
    v.sort();
    v.dedup();
    v
}

/// A filter's list: every one, or one value.
fn filter_combo(ui: &mut egui::Ui, id: &str, label: &str, all: &str, hint: &'static str, values: &[String], chosen: &mut String) {
    let shown = if chosen.is_empty() { all.to_string() } else { chosen.clone() };
    egui::ComboBox::new(id, label)
        .selected_text(shown)
        .show_ui(ui, |ui| {
            if ui.selectable_label(chosen.is_empty(), all).hint(hint).clicked() {
                chosen.clear();
            }
            for v in values {
                if ui.selectable_label(chosen == v, v).hint(hint).clicked() {
                    *chosen = v.clone();
                }
            }
        })
        .response
        .hint(hint);
}

/// Written music: the search, the sort and the filters, the list, and the chosen piece's details.
pub(super) fn written(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: Color32, wide: bool) {
    let hint = state.settings.hint_display;
    let lib = state.music_dev.readout.lib.clone();
    let scores: Vec<ScoreEntry> = lib.as_ref().map(|l| l.scores.clone()).unwrap_or_default();
    let mut dirty = false;
    widgets::subsection_header(ui, theme, accent, t("music_page.written"), t("music_page.written_note"));
    ui.label(RichText::new(t("music_page.written_intro")).color(theme.text_secondary()));
    ui.add_space(theme.spacing_xs);
    if scores.is_empty() {
        ui.label(RichText::new(t("music_page.written_none")).color(theme.text_muted()));
        return;
    }
    let composers = choices(&scores, |e| score_words(e).1);
    let eras = choices(&scores, era_words);
    {
        let q = &mut state.music_page.written_query;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(t("music_page.written_search")).color(theme.text_secondary()));
            ui.add(egui::TextEdit::singleline(&mut q.text).desired_width(260.0)).hint("music_page.written_search_hint");
            ui.add_space(theme.spacing_lg);
            egui::ComboBox::new("music_page_written_sort", t("music_page.written_sort"))
                .selected_text(t(q.sort.key()))
                .show_ui(ui, |ui| {
                    for s in WrittenSort::ALL {
                        ui.selectable_value(&mut q.sort, s, t(s.key())).hint("music_page.written_sort_hint");
                    }
                })
                .response
                .hint("music_page.written_sort_hint");
            ui.add_space(theme.spacing_lg);
            filter_combo(ui, "music_page_written_composer", t("music_page.written_composer_filter"), t("music_page.written_all_composers"), "music_page.written_composer_filter_hint", &composers, &mut q.composer);
            ui.add_space(theme.spacing_lg);
            filter_combo(ui, "music_page_written_era", t("music_page.written_era_filter"), t("music_page.written_all_eras"), "music_page.written_era_filter_hint", &eras, &mut q.era);
        });
    }
    widgets::setting_hint(ui, theme, hint, t("music_page.written_hint"));
    let order = catalogue(&scores, &state.music_page.written_query);
    ui.label(RichText::new(t("music_page.written_count").replace("{{n}}", &order.len().to_string()).replace("{{total}}", &scores.len().to_string())).size(theme.font_size_small).color(theme.text_muted()));
    // The details show the piece chosen, else the one playing, else the first in the list.
    let selected = state.music_page.written_selected.clone().filter(|id| scores.iter().any(|e| &e.id == id)).or_else(|| (!state.settings.music.written.is_empty()).then(|| state.settings.music.written.clone())).or_else(|| order.first().map(|&i| scores[i].id.clone()));
    let mut pick: Option<String> = None;
    // The list: true when Play or Stop changed the settings, and the row chosen.
    let list = |ui: &mut egui::Ui, state: &mut GuiState| -> (bool, Option<String>) {
        let (mut changed, mut chosen) = (false, None);
        widgets::card(ui, theme, |ui| {
            ui.set_min_width(ui.available_width());
            if order.is_empty() {
                ui.label(RichText::new(t("music_page.written_no_match")).color(theme.text_muted()));
            }
            for &i in &order {
                let e = &scores[i];
                let (title, composer) = score_words(e);
                ui.horizontal_wrapped(|ui| {
                    changed |= play_button(ui, state, e);
                    if ui.selectable_label(selected.as_deref() == Some(e.id.as_str()), RichText::new(&title).strong().color(theme.text_primary())).hint("music_page.written_row_hint").clicked() {
                        chosen = Some(e.id.clone());
                    }
                    let mut line = vec![composer];
                    line.push(era_words(e));
                    if e.length_s > 0.0 {
                        line.push(length_words(e.length_s));
                    }
                    ui.label(RichText::new(line.into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ")).color(theme.text_secondary()));
                    status(ui, theme, state, e, accent);
                });
            }
        });
        (changed, chosen)
    };
    let detail = |ui: &mut egui::Ui, state: &mut GuiState| -> bool {
        let mut changed = false;
        widgets::card(ui, theme, |ui| {
            ui.set_min_width(ui.available_width());
            match selected.as_deref().and_then(|id| scores.iter().find(|e| e.id == id)) {
                Some(e) => changed = details(ui, theme, state, e, accent),
                None => {
                    ui.label(RichText::new(t("music_page.detail_none")).color(theme.text_muted()));
                }
            }
        });
        changed
    };
    if wide {
        let gap = theme.spacing_md;
        let list_w = ((ui.available_width() - gap) * 0.55).max(360.0);
        let detail_w = (ui.available_width() - gap - list_w).max(300.0);
        // No height: each column grows with what it holds (the page scrolls; a scroll area's own height
        // is unbounded).
        let h = 0.0;
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(list_w, h), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_width(list_w);
                let (d, p) = list(ui, state);
                dirty |= d;
                pick = p;
            });
            ui.add_space(gap);
            ui.allocate_ui_with_layout(egui::vec2(detail_w, h), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.set_width(detail_w);
                dirty |= detail(ui, state);
            });
        });
    } else {
        let (d, p) = list(ui, state);
        dirty |= d;
        pick = p;
        ui.add_space(theme.spacing_sm);
        dirty |= detail(ui, state);
    }
    if let Some(id) = pick {
        state.music_page.written_selected = Some(id);
    }
    if dirty {
        state.settings_dirty = true;
    }
}

/// Play, or Stop for the piece the person chose. True when the settings changed.
fn play_button(ui: &mut egui::Ui, state: &mut GuiState, e: &ScoreEntry) -> bool {
    let m = &mut state.settings.music;
    if m.written == e.id {
        if ui.button(t("music_page.written_stop")).hint("music_page.written_stop_hint").clicked() {
            m.written.clear();
            return true;
        }
    } else if ui.button(t("music_page.written_play")).hint("music_page.written_play_hint").clicked() {
        m.written = e.id.clone();
        return true;
    }
    false
}

/// What the chosen piece is doing now (playing, or starting at the next bar), and why it could not
/// be read when it could not.
fn status(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, e: &ScoreEntry, accent: Color32) {
    if state.settings.music.written == e.id {
        let now = if state.music_dev.readout.written.as_deref() == Some(e.id.as_str()) { t("music_page.written_playing") } else { t("music_page.written_starting") };
        ui.label(RichText::new(now).color(accent));
    }
    if let Some((id, why)) = &state.music_dev.readout.written_error {
        if *id == e.id {
            ui.label(RichText::new(t("music_page.written_error").replace("{{why}}", why)).color(theme.warning()));
        }
    }
}

/// One piece's details: everything the index says about where it came from. True when Play or Stop
/// changed the settings.
fn details(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, e: &ScoreEntry, accent: Color32) -> bool {
    let (title, composer) = score_words(e);
    ui.label(RichText::new(&title).size(theme.font_size_heading).strong().color(theme.text_primary()));
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        changed = play_button(ui, state, e);
        status(ui, theme, state, e, accent);
    });
    let composer = if e.composer_years.is_empty() { composer } else { t("music_page.detail_composer_value").replace("{{composer}}", &composer).replace("{{years}}", &e.composer_years) };
    let row = |ui: &mut egui::Ui, label: &str, value: &str| {
        if !value.is_empty() {
            widgets::detail_row(ui, theme, label, value);
        }
    };
    row(ui, t("music_page.detail_composer"), &composer);
    row(ui, t("music_page.detail_era"), &era_words(e));
    if e.year > 0 {
        row(ui, t("music_page.detail_year"), &e.year.to_string());
    }
    if !e.key_root.is_empty() {
        let mode = t_dyn(&format!("music.mode.{}", e.key_mode));
        row(ui, t("music_page.detail_key"), &t("music_page.detail_key_value").replace("{{root}}", &e.key_root).replace("{{mode}}", mode));
    }
    if e.length_s > 0.0 {
        row(ui, t("music_page.detail_length"), &length_words(e.length_s));
    }
    row(ui, t("music_page.detail_instruments"), &instruments(e));
    row(ui, t("music_page.detail_edition"), &e.edition);
    row(ui, t("music_page.detail_source"), &e.source);
    row(ui, t("music_page.detail_typesetter"), &e.typesetter);
    row(ui, t("music_page.detail_licence"), &e.licence);
    if !e.tempo_words.is_empty() {
        row(ui, t("music_page.detail_tempo"), t_dyn(&e.tempo_words));
    }
    row(ui, t("music_page.detail_finding"), &e.tempo_finding);
    // The credit the finding asks for as a courtesy (the edition, and the typesetter who placed the
    // file in the public domain).
    if let Some(credit) = super::credit_words(e) {
        ui.label(RichText::new(credit).size(theme.font_size_small).color(theme.text_muted()));
    }
    if !e.source_url.is_empty() && ui.button(t("music_page.detail_open_source")).hint("music_page.detail_open_source_hint").clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(&e.source_url));
    }
    changed
}

//! THE NOW PLAYING STRIP (docs/design/music-page.md section 6; the operator, 2026-11-20: "We should
//! add a button to save a liked seed"): always in view along the top of the Music page. Pause or
//! play (the music on or off everywhere, Settings > Music's own switch), the previous and the next
//! piece, the sentence that says what plays and who chose it (`now_playing_words`, the very words
//! Settings > Music shows), chips for the genre, the mood and the seed that open their tab (for a
//! written piece its title, and BACK TO COMPOSED MUSIC: BUG-343), and LIKE: the piece shown, with
//! every change made to it, kept as a favourite in one click under a
//! name made for you. Nothing leaves the computer: it is the person's own favourites file.

use std::sync::Arc;

use egui::{Color32, RichText};
use humanity_core::i18n::t;
use humanity_music::favourite::{Favourite, SavedTune};

use super::{genre_label, mood_words, now_playing_words, score_words, written, Shown, Tab};
use crate::gui::theme::Theme;
use crate::gui::widgets::Button;
use crate::gui::GuiState;

/// The favourite Like would keep for the piece shown: its genre, mood, seed and tuning (by ids), and
/// a name made from its genre's words and its seed.
pub fn liked_favourite(state: &GuiState, sh: &Shown) -> Favourite {
    let lib = state.music_dev.readout.lib.clone();
    let tune = match lib.as_deref().and_then(|l| l.genre_index(&sh.genre).map(|g| (l, g))) {
        Some((l, g)) => SavedTune::from_tune(&sh.tune, l, g),
        None => SavedTune::default(),
    };
    let genre = match lib.as_deref().and_then(|l| l.genres.iter().find(|g| g.id == sh.genre)) {
        Some(g) => genre_label(&g.id, &g.words, &g.name),
        None => sh.genre.replace('_', " "),
    };
    let name = t("music_page.liked_name").replace("{{genre}}", &genre).replace("{{seed}}", &sh.seed.to_string());
    Favourite { id: 0, name, genre: sh.genre.clone(), mood: sh.mood.clone(), seed: sh.seed, tune }
}

/// The favourite that already keeps the piece shown (the same genre, mood, seed and tuning), if any.
pub fn already_liked(state: &GuiState, f: &Favourite) -> Option<u64> {
    state.music_page.favourites.favourites.iter().find(|x| x.genre == f.genre && x.mood == f.mood && x.seed == f.seed && x.tune == f.tune).map(|x| x.id)
}

/// Like: keep the piece shown in one click, or (already kept) open Your music with it marked. Never
/// removes one: an unlike by accident would lose a name the person gave it.
pub fn like(state: &mut GuiState, sh: &Shown) {
    let f = liked_favourite(state, sh);
    if let Some(id) = already_liked(state, &f) {
        state.music_page.marked = Some(id);
        state.music_page.tab = Tab::Yours;
        return;
    }
    let name = f.name.clone();
    let p = &mut state.music_page;
    let id = Arc::make_mut(&mut p.favourites).add(f);
    p.marked = Some(id);
    p.like_msg = Some(t("music_page.liked_msg").replace("{{name}}", &name));
    p.save_favourites();
}

/// The written piece before or after the one playing in the list as the person sorted and filtered it
/// (wrapping round at either end). True when it changed.
pub fn step_written(state: &mut GuiState, next: bool) -> bool {
    let scores = super::files::all_scores(state);
    let order = written::catalogue(&scores, &state.music_page.written_query);
    let Some(i) = order.iter().position(|&k| scores[k].id == state.settings.music.written) else { return false };
    let n = order.len();
    let j = if next { (i + 1) % n } else { (i + n - 1) % n };
    state.settings.music.written = scores[order[j]].id.clone();
    true
}

/// The strip: buttons, what plays, the chips and Like. Previous and Next change `sh` (a composed
/// piece's seed) or the person's written piece (the one before or after it in the Written music list).
pub(super) fn strip(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, sh: &mut Shown, accent: Color32) {
    let words = now_playing_words(&state.settings.music, &state.music_dev.readout);
    let written_id = state.settings.music.written.clone();
    let lib = state.music_dev.readout.lib.clone();
    let mut dirty = false;
    crate::gui::widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            let on = state.settings.music.music_on;
            let (label, hint) = if on { (t("music_page.pause"), "music_page.pause_hint") } else { (t("music_page.play"), "music_page.play_hint") };
            if Button::primary(label).hint(hint).show(ui, theme) {
                state.settings.music.music_on = !on;
                dirty = true;
            }
            let prev = Button::new(t("music_page.previous")).hint("music_page.prev_piece_hint").show(ui, theme);
            let next = Button::new(t("music_page.next")).hint("music_page.next_piece_hint").show(ui, theme);
            if prev || next {
                if written_id.is_empty() {
                    sh.seed = if next { sh.seed.saturating_add(1) } else { sh.seed.saturating_sub(1).max(1) };
                } else {
                    dirty |= step_written(state, next);
                }
            }
            ui.add_space(theme.spacing_sm);
            ui.label(RichText::new(words).strong().color(theme.text_primary()));
        });
        ui.horizontal_wrapped(|ui| {
            if written_id.is_empty() {
                let genre = match lib.as_deref().and_then(|l| l.genres.iter().find(|g| g.id == sh.genre)) {
                    Some(g) => genre_label(&g.id, &g.words, &g.name),
                    None => sh.genre.replace('_', " "),
                };
                if Button::ghost(&genre).hint("music_page.chip_genre_hint").show(ui, theme) {
                    state.music_page.tab = Tab::Explore;
                }
                if Button::ghost(&mood_words(lib.as_deref(), &sh.mood)).hint("music_page.chip_mood_hint").show(ui, theme) {
                    state.music_page.tab = Tab::Explore;
                }
                if Button::ghost(&t("music_page.chip_seed").replace("{{seed}}", &sh.seed.to_string())).hint("music_page.chip_seed_hint").show(ui, theme) {
                    state.music_page.tab = Tab::Explore;
                }
            } else {
                let title = super::files::entry_of(state, &written_id).map(|e| score_words(&e).0).unwrap_or_else(|| written_id.replace('_', " "));
                if Button::ghost(&title).hint("music_page.chip_written_hint").show(ui, theme) {
                    state.music_page.tab = Tab::Written;
                    state.music_page.written_selected = Some(written_id.clone());
                }
                // The way back to composed music, in view on every tab (BUG-343).
                if Button::new(t("music_page.composed_back")).hint("music_page.composed_back_hint").show(ui, theme) {
                    dirty |= super::back_to_composed(&mut state.settings.music);
                }
            }
            ui.add_space(theme.spacing_sm);
            if written_id.is_empty() {
                let f = liked_favourite(state, sh);
                let liked = already_liked(state, &f).is_some();
                let (label, hint) = if liked { (t("music_page.liked"), "music_page.liked_hint") } else { (t("music_page.like"), "music_page.like_hint") };
                if Button::primary(label).active(liked).hint(hint).show(ui, theme) {
                    like(state, sh);
                }
            } else {
                Button::primary(t("music_page.like")).disabled(true).hint("music_page.like_written_hint").show(ui, theme);
            }
            if let Some(m) = &state.music_page.like_msg {
                ui.label(RichText::new(m).size(theme.font_size_small).color(accent));
            }
        });
        if state.settings.music.music_on && !state.music_dev.readout.device {
            ui.label(RichText::new(t("music_page.no_device")).size(theme.font_size_small).color(theme.text_muted()));
        }
    });
    if dirty {
        state.settings_dirty = true;
    }
}

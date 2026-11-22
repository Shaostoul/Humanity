//! LIKE (docs/design/music-page.md section 6; the operator, 2026-11-20: "We should add a button to save a
//! liked seed"): the piece shown, with every change made to it, kept as a favourite in one click under
//! a name made for you, from the transport bar's heart (transport.rs) or the key L. Nothing leaves the
//! computer: it is the person's own favourites file. A written piece waits for playlists (version 3,
//! section 28.4, rung 2).

use std::sync::Arc;

use humanity_core::i18n::t;
use humanity_music::favourite::{Favourite, SavedTune};

use super::{genre_label, Shown, Tab};
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

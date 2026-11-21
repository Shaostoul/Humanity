//! BACK TO COMPOSED MUSIC, IN WORDS (BUG-343; the operator, 2026-11-21: "I can't seem to play the
//! procedural music any more"). The Listen tab says plainly which kind of music is the person's: a
//! written piece by its name, or composed music with its genre and mood; Explore says that its cards
//! play composed music again while a written piece plays; and the keyboard's N and P follow what plays
//! (the next written piece, then after Back to composed music the next seed). The runtime half (each
//! click heard as composed music in the score) is engine/music_tests_back.rs.

use egui::{Event, Key, Modifiers};
use humanity_core::i18n::t;

use super::tests::{click, frame, music_state, texts};
use super::Tab;
use crate::gui::widgets::tint_section::texts_at;

/// A key pressed and let go.
fn key(k: Key) -> Vec<Event> {
    vec![Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }, Event::Key { key: k, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE }]
}

/// THE PAGE SAYS WHICH KIND PLAYS, AND THE KEYS FOLLOW IT: with Ode to Joy chosen, Listen says it is
/// written music by its name and how to hear composed music again, Explore says its cards play
/// composed music again, and N moves to the next written piece; after Back to composed music, Listen
/// names composed music with its genre and mood, Explore's note is gone, and N moves to the next seed.
///
/// Seen RED 2026-11-21 against the page before the fix (no words for the kind of music): "Listen says
/// a written piece is the music, by its name".
#[test]
fn the_page_says_which_kind_of_music_plays_and_the_keys_follow_it() {
    let (mut state, lib) = music_state();
    let theme = crate::gui::theme::load_theme();
    let size = egui::vec2(1600.0, 6000.0);
    let title = t("music.score.ode_to_joy");
    state.settings.music.written = "ode_to_joy".into();
    state.music_page.tab = Tab::Listen;
    let page = |state: &mut crate::gui::GuiState, events: Vec<Event>| frame(size, events, |ctx| super::draw(ctx, &theme, state));
    let drawn = texts(&page(&mut state, Vec::new()));
    let written = t("music_page.kind_written").replace("{{title}}", title);
    assert!(drawn.contains(&written), "Listen says a written piece is the music, by its name");
    assert!(drawn.iter().any(|x| x == t("music_page.kind_back")), "and how to hear composed music again");
    state.music_page.tab = Tab::Explore;
    let drawn = texts(&page(&mut state, Vec::new()));
    assert!(drawn.contains(&t("music_page.explore_written_note").replace("{{title}}", title)), "Explore says its cards play composed music again");
    // N while a written piece is the music: the next written piece.
    page(&mut state, key(Key::N));
    assert!(!state.settings.music.written.is_empty() && state.settings.music.written != "ode_to_joy", "N plays the next written piece: {:?}", state.settings.music.written);
    // Back to composed music, from the strip.
    let out = page(&mut state, Vec::new());
    let back = *texts_at(&out, t("music_page.composed_back")).first().expect("the strip offers Back to composed music");
    page(&mut state, click(back.center()));
    assert!(state.settings.music.written.is_empty(), "Back to composed music clears the written piece");
    state.music_page.tab = Tab::Listen;
    let drawn = texts(&page(&mut state, Vec::new()));
    let sh = super::shown(&state.settings.music, &state.music_dev.readout);
    let genre = lib.genres.iter().find(|g| g.id == sh.genre).map(|g| super::genre_label(&g.id, &g.words, &g.name)).expect("the genre in the data");
    let composed = t("music_page.kind_composed").replace("{{genre}}", &genre).replace("{{mood}}", &super::mood_words(Some(&lib), &sh.mood));
    assert!(drawn.contains(&composed), "Listen names composed music with its genre and mood: {composed:?}");
    assert!(!drawn.iter().any(|x| x == t("music_page.composed_back")), "no way back offered while composed music is the music");
    state.music_page.tab = Tab::Explore;
    let drawn = texts(&page(&mut state, Vec::new()));
    let note = t("music_page.explore_written_note").split("{{title}}").next().unwrap_or_default();
    assert!(!note.is_empty() && !drawn.iter().any(|x| x.starts_with(note)), "Explore's note is gone with the written piece");
    // N while composed music is the music: the next seed.
    let seed = sh.seed;
    page(&mut state, key(Key::N));
    assert!(state.settings.music.written.is_empty(), "N keeps composed music");
    assert_eq!(state.settings.music.seed, seed + 1, "N plays the next seed");
}

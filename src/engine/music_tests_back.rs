// BACK TO COMPOSED MUSIC (BUG-343, the operator on v0.1568.0: "I can't seem to play the procedural
// music any more. I can click play 20 seconds but, then it seems to switch back to the sheet music.").
// A written piece chosen on the Written music list stays the person's choice in their settings, so
// these tests draw the real Music page over a runtime playing Ode to Joy, click what a person clicks
// to go back to composed music, hand the runtime the settings the page wrote, and read what the
// RUNTIME then plays (its readout), never only the setting the click wrote. And the 20-second Listen
// audition still goes back to exactly what played before it, the written piece included.

use super::*;
use crate::gui::pages::music::tests::{click, frame};
use crate::gui::pages::music::Tab;
use crate::gui::widgets::tint_section::texts_at;
use crate::gui::GuiState;

/// The music's data alone, quick (`quick_music`), with the shipped written pieces and sound styles.
fn quick_music_with_scores(tag: &str) -> humanity_core::test_temp::TempPath {
    let t = quick_music(tag);
    let data = t.join("data");
    let repo = Path::new(env!("HUMANITY_REPO_ROOT")).join("data/music");
    for d in ["scores", "styles"] {
        std::fs::create_dir_all(data.join("music").join(d)).unwrap();
        for e in std::fs::read_dir(repo.join(d)).unwrap().flatten() {
            if e.path().is_file() {
                std::fs::copy(e.path(), data.join("music").join(d).join(e.file_name())).unwrap();
            }
        }
    }
    t
}

/// A runtime playing Ode to Joy, chosen as a written piece (the game choosing the rest, as it does
/// out of the box), and the readout once it is heard.
fn ode_to_joy_playing(tag: &str) -> (humanity_core::test_temp::TempPath, Inputs, MusicRuntime, Offline, MusicReadout) {
    let t = quick_music_with_scores(tag);
    let mut inp = inputs(&t.join("data"));
    inp.settings.written = "ode_to_joy".into();
    let mut rt = MusicRuntime { seed: 20261121, ..Default::default() };
    let mut mix = Offline::default();
    let r = run_until(&mut rt, &mut mix, &inp, 4000, |r| r.written.as_deref() == Some("ode_to_joy") && r.score.is_some_and(|s| s.seconds > 0.2));
    (t, inp, rt, mix, r)
}

/// The Music page as the person sees it with that readout, on `tab`.
fn page_over(r: &MusicReadout, settings: &MusicSettings, tab: Tab) -> GuiState {
    let mut gui = GuiState::default();
    gui.music_page.fav_loaded = true;
    gui.settings.music = settings.clone();
    gui.music_dev.readout = r.clone();
    gui.music_page.tab = tab;
    gui
}

/// Draw the page (tall enough that nothing scrolls), the last frame with `events`.
fn draw(gui: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
    let theme = crate::gui::theme::load_theme();
    frame(egui::vec2(1600.0, 6000.0), events, |ctx| crate::gui::pages::music::draw(ctx, &theme, gui))
}

/// Where `words` is drawn on Explore in its genre cards (`moods: false`, above the moods' heading) or
/// its mood cards (below it): "Mothership calm" names both a genre and a mood.
fn card_at(out: &egui::FullOutput, words: &str, moods: bool) -> egui::Pos2 {
    let line = texts_at(out, humanity_core::i18n::t("music_page.moods_title")).first().expect("Explore has the moods' heading").center().y;
    texts_at(out, words).into_iter().find(|r| (r.center().y > line) == moods).unwrap_or_else(|| panic!("a card named {words:?}")).center()
}

/// Step until the score plays composed music (no written piece in it), or fail saying what plays.
fn run_until_composed(rt: &mut MusicRuntime, mix: &mut Offline, inp: &Inputs, what: &str) -> MusicReadout {
    let mut last = None;
    for _ in 0..3000 {
        let r = rt.step(Some(mix as &mut dyn Mixer), inp);
        let composed = r.written.is_none() && r.score.is_some_and(|s| s.seconds > 0.0) && r.playing.as_ref().is_some_and(|p| p.choice.as_ref().is_none_or(|c| c.written.is_none()));
        if composed {
            return r;
        }
        last = r.written.clone();
        rt.wait_for_build();
        mix.render(inp.dt);
    }
    panic!("{what}: composed music never played in 3000 frames; the runtime still plays the written piece {last:?}");
}

/// A GENRE CARD PLAYS COMPOSED MUSIC: with Ode to Joy playing, a click on the genre card already
/// marked chosen (a card that changes nothing else) makes the runtime play composed music in it.
///
/// Seen RED 2026-11-21 against the page before the fix (a click on a chosen card did nothing, and
/// the written piece stayed the choice): "a genre card: composed music never played in 3000 frames;
/// the runtime still plays the written piece Some(\"ode_to_joy\")".
#[test]
fn with_a_written_piece_playing_a_genre_card_plays_composed_music() {
    let (_t, mut inp, mut rt, mut mix, r) = ode_to_joy_playing("music_back_genre");
    let genre = inp.settings.genre.clone();
    let mut gui = page_over(&r, &inp.settings, Tab::Explore);
    let out = draw(&mut gui, Vec::new());
    let words = r.lib.as_deref().and_then(|l| l.genres.iter().find(|g| g.id == genre)).map(|g| crate::gui::pages::music::genre_label(&g.id, &g.words, &g.name)).expect("the chosen genre in the data");
    draw(&mut gui, click(card_at(&out, &words, false)));
    inp.settings = gui.settings.music.clone();
    run_until_composed(&mut rt, &mut mix, &inp, "a genre card");
    assert_eq!(rt.score_genres, vec![genre], "composed music in the genre clicked");
}

/// A MOOD CARD PLAYS COMPOSED MUSIC: with Ode to Joy playing, a click on another mood's card makes the
/// runtime play composed music, in that mood.
///
/// Seen RED 2026-11-21 against the page before the fix (the mood became the person's choice, but the
/// written piece stayed it): "a mood card: composed music never played in 3000 frames; the runtime
/// still plays the written piece Some(\"ode_to_joy\")".
#[test]
fn with_a_written_piece_playing_a_mood_card_plays_composed_music() {
    let (_t, mut inp, mut rt, mut mix, r) = ode_to_joy_playing("music_back_mood");
    let mut gui = page_over(&r, &inp.settings, Tab::Explore);
    let out = draw(&mut gui, Vec::new());
    let tension = crate::gui::pages::music::mood_words(r.lib.as_deref(), "tension");
    draw(&mut gui, click(card_at(&out, &tension, true)));
    inp.settings = gui.settings.music.clone();
    let r = run_until_composed(&mut rt, &mut mix, &inp, "a mood card");
    assert_eq!(r.playing.as_ref().and_then(|p| p.choice.as_ref()).and_then(|c| c.mood.clone()).as_deref(), Some("tension"), "composed music in the mood clicked");
}

/// THE STRIP'S WAY BACK: with Ode to Joy playing, the Now playing strip offers Back to composed music
/// (on every tab: here Listen), and a click on it makes the runtime play composed music.
///
/// Seen RED 2026-11-21 against the strip before the fix (it had no such control): "the strip offers
/// Back to composed music while a written piece plays".
#[test]
fn with_a_written_piece_playing_the_strips_back_control_plays_composed_music() {
    let (_t, mut inp, mut rt, mut mix, r) = ode_to_joy_playing("music_back_strip");
    let mut gui = page_over(&r, &inp.settings, Tab::Listen);
    let out = draw(&mut gui, Vec::new());
    let back = *texts_at(&out, humanity_core::i18n::t("music_page.composed_back")).first().expect("the strip offers Back to composed music while a written piece plays");
    draw(&mut gui, click(back.center()));
    inp.settings = gui.settings.music.clone();
    run_until_composed(&mut rt, &mut mix, &inp, "the strip's Back to composed music");
    assert!(gui.settings_dirty, "the choice is saved");
}

/// THE AUDITION STILL GOES BACK TO THE WRITTEN PIECE: with Ode to Joy playing, a genre card's Listen
/// plays 20 seconds of composed music, then Ode to Joy comes back, and the written piece is still the
/// person's choice (Listen is not a click on the card).
///
/// Seen RED 2026-11-21 with Listen made to count as a click on its card (the written choice cleared
/// with it): "Listen leaves the written piece chosen (left: \"\", right: \"ode_to_joy\")".
#[test]
fn an_audition_over_a_written_piece_goes_back_to_it() {
    let (_t, mut inp, mut rt, mut mix, r) = ode_to_joy_playing("music_back_audition");
    let mut gui = page_over(&r, &inp.settings, Tab::Explore);
    let out = draw(&mut gui, Vec::new());
    let listen = humanity_core::i18n::t("music_page.listen").replace("{{n}}", &(crate::gui::pages::music::AUDITION_S as u32).to_string());
    let first = *texts_at(&out, &listen).first().expect("each card has Listen");
    draw(&mut gui, click(first.center()));
    assert_eq!(gui.settings.music.written, "ode_to_joy", "Listen leaves the written piece chosen");
    inp.settings = gui.settings.music.clone();
    inp.audition = gui.music_page.audition.as_ref().map(|a| a.choice.clone());
    assert!(inp.audition.is_some(), "Listen asks for an audition");
    let r = run_until(&mut rt, &mut mix, &inp, 4000, |r| r.audition_s.is_some());
    assert!(r.written.is_none(), "the audition plays composed music: {:?}", r.written);
    run_until(&mut rt, &mut mix, &inp, 4000, |r| r.audition_done);
    // The engine's frame ends it (engine/music.rs: `audition_done` clears the page's audition).
    inp.audition = None;
    run_until(&mut rt, &mut mix, &inp, 4000, |r| r.written.as_deref() == Some("ode_to_joy") && r.score.is_some_and(|s| s.seconds > 0.0));
}

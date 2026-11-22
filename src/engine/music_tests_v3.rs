// THE MUSIC PAGE, VERSION 3 RUNG 1, HEARD IN THE RUNTIME (docs/design/music-page.md sections 28 and
// 29). The real page is drawn over a runtime playing real scores offline; each control of the transport
// bar is clicked where the page drew it (egui's own record of the button, `Context::read_response`),
// the settings the page wrote are handed to the runtime, and what the RUNTIME then plays is read from
// its readout, never only the setting or the flag the click wrote. The one list's frame
// (`MusicPage::tick_queue`, the engine's music frame calls it page open or not) runs between steps as
// the engine runs it.

use super::*;
use crate::gui::pages::music::queue::{next_station_seed, Item, Repeat};
use crate::gui::pages::music::tests_v3::draw_kept;
use crate::gui::pages::music::transport::{button_id, LIKE, MUTE, NEXT, PLAY, PREV, REPEAT, SEEK_ID, SHUFFLE, UP_NEXT};
use crate::gui::pages::music::Tab;
use crate::gui::GuiState;

/// The music's data alone, quick, with the shipped written pieces (as music_tests_back.rs makes it).
fn quick_with_scores(tag: &str) -> humanity_core::test_temp::TempPath {
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

/// The page as the person sees it over readout `r`.
fn page_over(r: &MusicReadout, settings: &MusicSettings) -> GuiState {
    let mut gui = GuiState::default();
    gui.music_page.fav_loaded = true;
    gui.settings.music = settings.clone();
    gui.music_dev.readout = r.clone();
    gui.music_page.tab = Tab::NowPlaying;
    gui
}

const PAGE: (f32, f32) = (1600.0, 1000.0);

/// Click the transport bar's button `id` (drawn first, so its place is known), with the readout `r`.
fn click_button(gui: &mut GuiState, r: &MusicReadout, id: &str) {
    gui.music_dev.readout = r.clone();
    let size = egui::vec2(PAGE.0, PAGE.1);
    let (_, ctx) = draw_kept(gui, size, Vec::new(), None);
    let rect = ctx.read_response(button_id(id)).unwrap_or_else(|| panic!("the bar draws {id}")).rect;
    draw_kept(gui, size, crate::gui::pages::music::tests::click(rect.center()), Some(ctx));
}

/// Hand the page's settings and seek to the runtime, then step until `done`, running the one list's
/// frame between steps as the engine does.
fn hear(rt: &mut MusicRuntime, mix: &mut Offline, inp: &mut Inputs, gui: &mut GuiState, max: usize, what: &str, done: impl Fn(&MusicReadout) -> bool) -> MusicReadout {
    for _ in 0..max {
        inp.settings = gui.settings.music.clone();
        inp.music_volume = gui.settings.music_volume;
        inp.seek = gui.music_page.seek;
        let r = rt.step(Some(mix as &mut dyn Mixer), inp);
        gui.music_dev.readout = r.clone();
        gui.music_page.tick_queue(&mut gui.settings.music, &r);
        if done(&r) {
            return r;
        }
        rt.wait_for_build();
        mix.render(inp.dt);
    }
    panic!("{what}: not heard in {max} frames");
}

/// The seed the runtime's playing choice composes (0: none, or a written piece).
fn seed_of(r: &MusicReadout) -> u64 {
    r.playing.as_ref().and_then(|p| p.choice.as_ref()).filter(|c| c.written.is_none()).map(|c| c.seed).unwrap_or(0)
}

/// A runtime playing composed piece `seed` of the person's (four minutes long), its page with `next`
/// up next.
fn composed_with_next(tag: &str, seed: u64, next: u64) -> (humanity_core::test_temp::TempPath, Inputs, MusicRuntime, Offline, GuiState, MusicReadout) {
    let t = quick_with_scores(tag);
    let mut inp = inputs(&t.join("data"));
    inp.settings.game_chooses = false;
    inp.settings.seed = seed;
    inp.settings.minutes = 4.0;
    let mut rt = MusicRuntime { seed: 20261121, ..Default::default() };
    let mut mix = Offline::default();
    let mut gui = page_over(&MusicReadout::default(), &inp.settings);
    gui.settings.music_volume = inp.music_volume;
    let e = 8.0;
    let s = &gui.settings.music;
    let now = Item::Composed { genre: s.genre.clone(), mood: s.mood.clone(), seed, tune: Default::default(), minutes: 4.0 };
    gui.music_page.queue.play_now(now, &mut gui.settings.music, e);
    let s = &gui.settings.music;
    gui.music_page.queue.add(Item::Composed { genre: s.genre.clone(), mood: s.mood.clone(), seed: next, tune: Default::default(), minutes: 4.0 });
    let r = hear(&mut rt, &mut mix, &mut inp, &mut gui, 4000, "the first piece", |r| seed_of(r) == seed && r.score.is_some_and(|s| s.seconds > 0.2));
    (t, inp, rt, mix, gui, r)
}

/// EACH ICON BUTTON DOES ITS ACTION, HEARD IN THE RUNTIME (section 28.5): with composed piece 5 playing
/// and piece 6 up next, a click on Next makes the runtime play piece 6, Previous piece 5 again, Pause
/// takes the score's gain to silence and Play brings it back, the speaker silences the music and brings
/// it back; Shuffle and Repeat turn their modes on (heard in their own tests), Like keeps the piece as a
/// favourite, and Up next opens the list.
///
/// Seen RED 2026-11-21 with Next's click calling Previous: "Next plays the next item: not heard in 3000
/// frames".
#[test]
fn each_transport_button_does_its_action_through_the_engine() {
    let (_t, mut inp, mut rt, mut mix, mut gui, r) = composed_with_next("music_v3_buttons", 5, 6);
    click_button(&mut gui, &r, NEXT);
    let r = hear(&mut rt, &mut mix, &mut inp, &mut gui, 3000, "Next plays the next item", |r| seed_of(r) == 6);
    click_button(&mut gui, &r, PREV);
    let r = hear(&mut rt, &mut mix, &mut inp, &mut gui, 3000, "Previous plays the item before again", |r| seed_of(r) == 5);
    click_button(&mut gui, &r, PLAY);
    assert!(!gui.settings.music.music_on, "the bar's Pause turns the music off");
    let r = hear(&mut rt, &mut mix, &mut inp, &mut gui, 3000, "Pause silences the score", |r| r.score.is_some_and(|s| s.gain < 0.01));
    click_button(&mut gui, &r, PLAY);
    let r = hear(&mut rt, &mut mix, &mut inp, &mut gui, 3000, "Play brings the score back", |r| r.score.is_some_and(|s| s.gain > 0.2));
    click_button(&mut gui, &r, MUTE);
    let r = hear(&mut rt, &mut mix, &mut inp, &mut gui, 3000, "the speaker silences the music", |r| r.score.is_some_and(|s| s.gain < 0.01));
    click_button(&mut gui, &r, MUTE);
    let r = hear(&mut rt, &mut mix, &mut inp, &mut gui, 3000, "a second click brings it back", |r| r.score.is_some_and(|s| s.gain > 0.2));
    click_button(&mut gui, &r, SHUFFLE);
    assert!(gui.music_page.queue.shuffle, "Shuffle turns shuffle on");
    click_button(&mut gui, &r, REPEAT);
    assert_eq!(gui.music_page.queue.repeat, Repeat::All, "Repeat steps to all");
    click_button(&mut gui, &r, LIKE);
    assert!(gui.music_page.favourites.favourites.iter().any(|f| f.seed == 5), "Like keeps the piece playing as a favourite");
    click_button(&mut gui, &r, UP_NEXT);
    assert!(gui.music_page.up_next_open, "Up next opens the list");
    let (out, _) = draw_kept(&mut gui, egui::vec2(PAGE.0, PAGE.1), Vec::new(), None);
    assert!(crate::gui::pages::music::tests::texts(&out).iter().any(|x| x == humanity_core::i18n::t("music_page.q_title")), "the list is drawn above the bar");
}

/// NEXT AND PREVIOUS STEP THE SAME FOR EVERY KIND (section 28.3): Ode to Joy playing, a composed piece up
/// next and a station at the end: Next plays the composed piece, Next again the station's next piece,
/// Previous the composed piece again, Previous Ode to Joy again, each heard in the runtime.
///
/// Seen RED 2026-11-21 with Next on an empty Up next playing the item again instead of the station: "Next
/// into the station: not heard in 3000 frames".
#[test]
fn next_and_previous_step_the_same_for_written_composed_and_into_the_station() {
    let t = quick_with_scores("music_v3_kinds");
    let mut inp = inputs(&t.join("data"));
    let mut rt = MusicRuntime { seed: 20261121, ..Default::default() };
    let mut mix = Offline::default();
    let mut gui = page_over(&MusicReadout::default(), &inp.settings);
    let genre = gui.settings.music.genre.clone();
    let q = &mut gui.music_page.queue;
    q.play_now(Item::Written { id: "ode_to_joy".into() }, &mut gui.settings.music, 8.0);
    q.add(Item::Composed { genre: genre.clone(), mood: "mothership_calm".into(), seed: 77, tune: Default::default(), minutes: 4.0 });
    q.station = Some(Item::Station { genre: genre.clone(), mood: "mothership_calm".into(), seed: 500, tune: Default::default() });
    hear(&mut rt, &mut mix, &mut inp, &mut gui, 4000, "Ode to Joy", |r| r.written.as_deref() == Some("ode_to_joy"));
    crate::gui::pages::music::transport::next(&mut gui);
    hear(&mut rt, &mut mix, &mut inp, &mut gui, 3000, "Next from a written piece to a composed one", |r| r.written.is_none() && seed_of(r) == 77);
    crate::gui::pages::music::transport::next(&mut gui);
    let station = next_station_seed(500);
    let r = hear(&mut rt, &mut mix, &mut inp, &mut gui, 3000, "Next into the station", |r| seed_of(r) == station);
    assert_eq!(r.playing.as_ref().and_then(|p| p.choice.as_ref()).map(|c| c.genre.clone()), Some(genre), "the station's genre");
    crate::gui::pages::music::transport::previous(&mut gui);
    hear(&mut rt, &mut mix, &mut inp, &mut gui, 3000, "Previous from the station to the composed piece", |r| seed_of(r) == 77);
    crate::gui::pages::music::transport::previous(&mut gui);
    hear(&mut rt, &mut mix, &mut inp, &mut gui, 3000, "Previous back to the written piece", |r| r.written.as_deref() == Some("ode_to_joy"));
}

/// REPEAT ONE STARTS A COMPOSED PIECE AGAIN; REPEAT OFF MOVES ON WHEN IT ENDS (section 28.3): piece 5,
/// six seconds long, with piece 6 up next: with repeat one, after its six seconds the runtime is back at
/// its start and still playing piece 5; with repeat off, when it ends the runtime plays piece 6.
///
/// Seen RED 2026-11-21 with repeat one treated as off in `Queue::tick`: "repeat one keeps piece 5
/// playing (left: 6, right: 5)".
#[test]
fn repeat_one_starts_a_piece_again_and_repeat_off_moves_on() {
    let (_t, mut inp, mut rt, mut mix, mut gui, _) = composed_with_next("music_v3_repeat", 5, 6);
    // Six seconds long: the item and the score's choice both.
    if let Some(Item::Composed { minutes, .. }) = gui.music_page.queue.now.as_mut() {
        *minutes = 0.1;
    }
    for it in gui.music_page.queue.up_next.iter_mut() {
        if let Item::Composed { minutes, .. } = it {
            *minutes = 0.1;
        }
    }
    gui.settings.music.minutes = 0.1;
    gui.music_page.queue.repeat = Repeat::One;
    hear(&mut rt, &mut mix, &mut inp, &mut gui, 3000, "piece 5 at six seconds long", |r| r.playing.as_ref().and_then(|p| p.choice.as_ref()).is_some_and(|c| c.length_s == Some(6.0) && c.seed == 5) && r.score.is_some_and(|s| s.seconds > 0.5 && s.seconds < 3.0));
    let seek0 = gui.music_page.seek.0;
    // To its end, then on: the music is back near the start, and still piece 5.
    hear(&mut rt, &mut mix, &mut inp, &mut gui, 3000, "piece 5 nears its end", |r| r.score.is_some_and(|s| s.seconds > 5.8));
    let r = hear(&mut rt, &mut mix, &mut inp, &mut gui, 3000, "the music goes back near a start", |r| r.score.is_some_and(|s| s.seconds < 3.0));
    assert_eq!(seed_of(&r), 5, "repeat one keeps piece 5 playing");
    assert!(gui.music_page.seek.0 != seek0, "repeat one sought the piece's first bar");
    gui.music_page.queue.repeat = Repeat::Off;
    hear(&mut rt, &mut mix, &mut inp, &mut gui, 3000, "repeat off moves on to piece 6 when piece 5 ends", |r| seed_of(r) == 6);
}

/// THE SEEK BAR SEEKS (section 28.5): a click halfway along the bar moves a composed piece (four minutes)
/// to the bar holding two minutes, and a written piece to the bar holding half its length, each heard on
/// the runtime's own clock.
///
/// Seen RED 2026-11-21 with `seek_to_time` doing nothing: "a click halfway along moves the composed
/// piece to two minutes: not heard in 600 frames".
#[test]
fn the_seek_bar_seeks_a_composed_and_a_written_piece() {
    let (_t, mut inp, mut rt, mut mix, mut gui, r) = composed_with_next("music_v3_seek", 5, 6);
    let click_half = |gui: &mut GuiState, r: &MusicReadout| {
        gui.music_dev.readout = r.clone();
        let size = egui::vec2(PAGE.0, PAGE.1);
        let (_, ctx) = draw_kept(gui, size, Vec::new(), None);
        let bar = ctx.read_response(egui::Id::new(SEEK_ID)).expect("the bar draws the seek bar").rect;
        draw_kept(gui, size, crate::gui::pages::music::tests::click(bar.center()), Some(ctx));
    };
    click_half(&mut gui, &r);
    let r = hear(&mut rt, &mut mix, &mut inp, &mut gui, 600, "a click halfway along moves the composed piece to two minutes", |r| r.score.is_some_and(|s| s.seconds > 100.0));
    let at = r.score.unwrap().seconds;
    assert!((110.0..=125.0).contains(&at), "the bar holding two minutes: {at:.1} s");
    // A written piece.
    let q = &mut gui.music_page.queue;
    q.play_now(Item::Written { id: "ode_to_joy".into() }, &mut gui.settings.music, 8.0);
    let r = hear(&mut rt, &mut mix, &mut inp, &mut gui, 4000, "Ode to Joy", |r| r.written.as_deref() == Some("ode_to_joy") && r.score.is_some_and(|s| s.seconds > 0.2 && s.seconds < 2.0));
    let len = r.written_piece.as_ref().map(|p| p.timeline.length_s).expect("the written piece");
    click_half(&mut gui, &r);
    let r = hear(&mut rt, &mut mix, &mut inp, &mut gui, 600, "a click halfway along moves the written piece to its middle", |r| r.score.is_some_and(|s| s.seconds > len * 0.3));
    let at = r.score.unwrap().seconds;
    assert!(at > len * 0.35 && at <= len * 0.55, "the bar holding half of {len:.1} s: {at:.1} s");
}

/// DANGER BREAKS IN OVER A WRITTEN PIECE, AND IT COMES BACK AT ITS BAR (section 28.6; the operator: "We
/// still want danger music breaking in"): Ode to Joy playing past its second bar; the danger rises and
/// the game's composed music in the danger mood plays in its place (the one list waiting, nothing
/// moved on); the danger passes and Ode to Joy comes back at the bar it left, not from its start.
///
/// Seen RED 2026-11-21 with `choice_now` never breaking in (`danger` false): "the danger breaks in over
/// the written piece: not heard in 3000 frames"; and with the return to the bar switched off (`back`
/// always None in the runtime's take-over): "Ode to Joy comes back at the bar it left (2), not from its
/// start: bar 0".
#[test]
fn danger_breaks_in_over_a_written_piece_and_it_comes_back_at_its_bar() {
    let t = quick_with_scores("music_v3_danger");
    let mut inp = inputs(&t.join("data"));
    let mut rt = MusicRuntime { seed: 20261121, ..Default::default() };
    let mut mix = Offline::default();
    let mut gui = page_over(&MusicReadout::default(), &inp.settings);
    gui.music_page.queue.play_now(Item::Written { id: "ode_to_joy".into() }, &mut gui.settings.music, 8.0);
    let r = hear(&mut rt, &mut mix, &mut inp, &mut gui, 4000, "Ode to Joy past bar 2", |r| r.written.as_deref() == Some("ode_to_joy") && r.score.is_some_and(|s| s.bar >= 2));
    let left = r.score.unwrap().bar;
    inp.intensity = 0.8;
    let r = hear(&mut rt, &mut mix, &mut inp, &mut gui, 3000, "the danger breaks in over the written piece", |r| r.danger && r.written.is_none() && r.score.is_some_and(|s| s.seconds > 0.0) && r.mood == "action");
    assert!(r.danger, "the readout says the danger plays");
    assert_eq!(gui.music_page.queue.now, Some(Item::Written { id: "ode_to_joy".into() }), "the one list waits while the danger plays");
    inp.intensity = 0.0;
    // (Once the returning score has rendered: a score's status is written as it renders, so the frame
    // it starts it still reads as the start, bar 0 and 0 seconds.)
    let r = hear(&mut rt, &mut mix, &mut inp, &mut gui, 3000, "Ode to Joy comes back", |r| r.written.as_deref() == Some("ode_to_joy") && r.score.is_some_and(|s| s.seconds > 0.0));
    let back = r.score.unwrap().bar;
    assert!(back >= left, "Ode to Joy comes back at the bar it left ({left}), not from its start: bar {back}");
}

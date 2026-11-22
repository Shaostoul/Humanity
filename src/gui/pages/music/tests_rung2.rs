//! THE MUSIC PAGE, RUNG 2 (docs/design/music-page.md sections 7, 8, 9, 12, 15a and 16): the mixer's
//! Solo heard in the rendered audio, the pieces heard kept across a restart and Clear, a click on a
//! bar seeking the written piece, a MIDI file refused in words, and the keyboard shortcuts held back
//! while a text box has the keyboard. Each test draws the real page headlessly and measures what the
//! page's action PRODUCES (the score's samples, the file on disk, the score's clock), never only the
//! value a control was set to.

use std::sync::Arc;

use egui::{Event, Key, Modifiers, Pos2};
use humanity_music::sheet::Sheet;
use humanity_music::{MusicContext, Resolved, SampleLibrary, ScoreEngine};

use super::tests::{click, music_state, text_rects};
use super::Tab;
use crate::gui::GuiState;

/// The operator's screen in points (2560 by 1440 at 100%).
const SCREEN: egui::Vec2 = egui::vec2(2560.0, 1440.0);

/// Draw the Music page in one context for as many frames as `events` has, each frame with its events;
/// `before(frame, ctx)` runs at the start of each frame. The last frame's output.
fn frames(state: &mut GuiState, events: Vec<Vec<Event>>, before: impl FnMut(usize, &egui::Context)) -> egui::FullOutput {
    frames_at(SCREEN, state, events, before)
}

/// [`frames`] in a window of `size` points (a tall one shows a whole tab without scrolling).
fn frames_at(size: egui::Vec2, state: &mut GuiState, events: Vec<Vec<Event>>, mut before: impl FnMut(usize, &egui::Context)) -> egui::FullOutput {
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    let mut out = None;
    for (i, ev) in events.into_iter().enumerate() {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, size)), time: Some(i as f64 / 60.0), events: ev, ..Default::default() };
        out = Some(ctx.run(input, |ctx| {
            before(i, ctx);
            super::draw(ctx, &theme, state);
        }));
    }
    out.unwrap()
}

fn key(k: Key) -> Vec<Event> {
    vec![Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]
}

/// Electro swing, seed 5, in the calm mood, as the person's choice.
fn swing_chosen() -> (GuiState, Arc<humanity_music::MusicLibrary>) {
    let (mut state, lib) = music_state();
    let s = &mut state.settings.music;
    s.game_chooses = false;
    s.genre = "electro_swing".into();
    s.mood = "mothership_calm".into();
    s.seed = 5;
    (state, lib)
}

/// Render 6 seconds of what the person's settings choose, as the score would play it.
fn render_choice(state: &GuiState, lib: &humanity_music::MusicLibrary) -> Vec<[f32; 2]> {
    let c = super::chosen(&state.settings.music, Some(lib)).expect("a choice");
    let g = lib.genre_index(&c.genre).unwrap() as u16;
    let mood = lib.mood_index(c.mood.as_deref().unwrap()).unwrap() as u16;
    render_tune(lib, MusicContext { mood, genre_a: g, genre_b: g, intensity: c.intensity.unwrap_or(0.3), tune: c.tune, ..Default::default() }, c.seed)
}

fn render_tune(lib: &humanity_music::MusicLibrary, ctx: MusicContext, seed: u64) -> Vec<[f32; 2]> {
    let mut e = ScoreEngine::new(Resolved::new(lib, &SampleLibrary::new()).unwrap(), 48000.0, seed, ctx);
    let mut buf = vec![[0.0f32; 2]; 128];
    let mut all = Vec::new();
    for _ in 0..(6.0 * 48000.0 / 128.0) as usize {
        e.render(&mut buf);
        all.extend_from_slice(&buf);
    }
    all
}

/// THE MIXER'S SOLO SILENCES THE OTHER PARTS IN WHAT THE SCORE PLAYS (music-page.md section 8): on the
/// Listen tab, a click on the Solo button in the bass's strip (found under the fader named "Bass" in
/// what the page drew) makes the person's choice render sample for sample as the same piece with every
/// other part muted, and unlike the piece before the click.
///
/// Seen RED 2026-11-20 with the strips' Solo button left out (as rung 1's parts grid had none): "the
/// bass's strip draws a Solo button". (A first version took the first "Bass" the page drew, the
/// visualizer's, and clicked the choir's Solo: the button is now found under the fader's own name.)
#[test]
fn the_mixers_solo_silences_the_other_parts_in_what_plays() {
    let (mut state, lib) = swing_chosen();
    let before = render_choice(&state, &lib);
    let out = frames(&mut state, vec![vec![]; 3], |_, _| {});
    // The bass's strip: its fader's name and, just under it in the same column, a Solo button (the
    // word "Bass" is drawn elsewhere on the page too).
    let rects = text_rects(&out);
    let solos: Vec<egui::Rect> = rects.iter().filter(|(w, _)| w == humanity_core::i18n::t("music_page.solo")).map(|(_, r)| *r).collect();
    let solo = rects
        .iter()
        .filter(|(w, _)| w == super::role_words(1))
        .filter_map(|(_, b)| solos.iter().find(|s| (s.center().x - b.center().x).abs() < super::parts::STRIP_W * 0.5 && s.top() > b.top() && s.top() - b.top() < 120.0).copied())
        .next()
        .expect("the bass's strip draws a Solo button");
    frames(&mut state, vec![vec![], vec![], click(solo.center())], |_, _| {});
    assert_eq!(state.settings.music.tune.soloed, vec!["bass".to_string()], "the click soloed the bass");
    let after = render_choice(&state, &lib);
    let c = super::chosen(&state.settings.music, Some(&lib)).unwrap();
    let alone = {
        let g = lib.genre_index(&c.genre).unwrap() as u16;
        let mood = lib.mood_index(c.mood.as_deref().unwrap()).unwrap() as u16;
        let tune = humanity_music::tune::Tune { mute: !(1u8 << 1), ..Default::default() };
        render_tune(&lib, MusicContext { mood, genre_a: g, genre_b: g, intensity: c.intensity.unwrap_or(0.3), tune, ..Default::default() }, c.seed)
    };
    assert_eq!(after.iter().zip(&alone).position(|(a, b)| a != b), None, "with the bass soloed the score plays only the bass");
    assert!(before.iter().zip(&after).any(|(a, b)| a != b), "and that is not what played before the click");
}

/// THE PIECES HEARD SURVIVE A RESTART, AND CLEAR EMPTIES THEM (music-page.md section 9; the operator,
/// 2026-11-20: "Yes"): two pieces heard are written to the person's file as they begin; a page started
/// afresh (a restart) reads the same list back; the Clear button on Explore empties the list and the
/// file.
///
/// Seen RED 2026-11-20 with the list kept in memory only (the page's file write left out, as rung 1
/// had it): "a restart reads the pieces heard back, left: [], right: [\"fur_elise\", \"ode_to_joy\"]".
#[test]
fn the_pieces_heard_survive_a_restart_and_clear_empties_them() {
    let dir = std::env::temp_dir().join(format!("hos-music-heard-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("heard.json");
    let (mut state, _lib) = music_state();
    state.music_page.heard_path = Some(path.clone());
    for id in ["ode_to_joy", "fur_elise"] {
        state.music_dev.readout.written = Some(id.into());
        let (s, r) = (state.settings.music.clone(), state.music_dev.readout.clone());
        state.music_page.note_heard(&s, &r);
    }
    let ids = |list: &[super::Heard]| list.iter().filter_map(|h| h.written.clone()).collect::<Vec<_>>();
    let back = super::history::load(&path).unwrap();
    assert_eq!(ids(&back), vec!["fur_elise".to_string(), "ode_to_joy".to_string()], "a restart reads the pieces heard back");
    // A page started afresh, its list read from the file.
    let (mut fresh, _) = music_state();
    fresh.music_page.heard = back;
    fresh.music_page.heard_path = Some(path.clone());
    fresh.music_page.tab = Tab::Explore;
    fresh.music_dev.readout.written = None;
    let tall = egui::vec2(2560.0, 6000.0);
    let out = frames_at(tall, &mut fresh, vec![vec![]; 3], |_, _| {});
    let clear = text_rects(&out).into_iter().find(|(w, _)| w == humanity_core::i18n::t("music_page.heard_clear")).map(|(_, r)| r.center());
    let clear = clear.expect("Explore draws Clear beside the pieces heard");
    frames_at(tall, &mut fresh, vec![vec![], vec![], click(clear)], |_, _| {});
    assert!(fresh.music_page.heard.is_empty(), "Clear empties the list");
    assert!(super::history::load(&path).unwrap().is_empty(), "and the file");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A CLICK ON BAR N PLAYS THE WRITTEN PIECE FROM BAR N (music-page.md section 7): Für Elise playing,
/// a click inside the bar the roll numbers 6 asks the score to seek there, and the score given that
/// request (in its context, as the engine sends it) stands at that bar's start on the piece's own
/// timeline.
///
/// Seen RED 2026-11-20 with the roll's click asking for no seek (as rung 1's roll, which only showed):
/// "the click asks for a seek to the bar it landed on, left: (0, 0), right: (1, 5)".
#[test]
fn a_click_on_a_bar_seeks_the_written_piece_to_it() {
    let (mut state, lib) = music_state();
    let data = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let piece = Arc::new(humanity_music::scores::load_piece(&data, lib.score("fur_elise").unwrap()).unwrap());
    state.settings.music.written = "fur_elise".into();
    state.music_dev.readout.written = Some("fur_elise".into());
    state.music_dev.readout.written_piece = Some(piece.clone());
    state.music_dev.readout.score = Some(humanity_music::MusicStatus { seconds: 0.5, ..Default::default() });
    state.music_page.roll_bars = 8;
    let out = frames(&mut state, vec![vec![]; 3], |_, _| {});
    // The bar number 6 as drawn, and the next bar's number: a click between them is inside bar 6.
    let rects = text_rects(&out);
    let six = rects.iter().find(|(w, _)| w == "6").map(|(_, r)| *r).expect("the roll numbers bar 6");
    let seven = rects.iter().find(|(w, r)| w == "7" && (r.center().y - six.center().y).abs() < 2.0).map(|(_, r)| *r).expect("and bar 7");
    let at = Pos2::new((six.left() + seven.left()) * 0.5, six.bottom() + 60.0);
    frames(&mut state, vec![vec![], vec![], click(at)], |_, _| {});
    assert_eq!(state.music_page.seek, (1, 5), "the click asks for a seek to the bar it landed on");
    let sheet = Sheet::from_timeline(&piece.timeline, &piece.staff_roles());
    let r = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
    let player = piece.player(&r, 48000.0, 432.0);
    let mut e = ScoreEngine::written(r, 48000.0, 1, MusicContext::default(), player);
    let mut buf = vec![[0.0f32; 2]; 128];
    e.render(&mut buf);
    e.set_context(MusicContext { seek: state.music_page.seek.0, seek_bar: state.music_page.seek.1, ..Default::default() });
    e.render(&mut buf);
    let want = sheet.bars[5];
    assert!((e.status().seconds - want).abs() < 0.01, "the score stands at bar 6's start ({want:.2} s): {:.2}", e.status().seconds);
}

/// A MALFORMED MIDI FILE IS REFUSED, IN WORDS ON THE PAGE (music-page.md section 15a): a text file
/// named .mid opened from the Written music tab adds nothing and the page says why in a sentence.
#[test]
fn a_malformed_midi_file_is_refused_with_words_on_the_page() {
    let dir = std::env::temp_dir().join(format!("hos-music-import-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let bad = dir.join("shopping.mid");
    std::fs::write(&bad, "eggs, flour, milk").unwrap();
    let (mut state, _) = music_state();
    state.music_page.own_dir = Some(dir.join("scores"));
    let words = super::files::import_file(&mut state, &bad);
    assert!(words.starts_with(&humanity_core::i18n::t("music_page.import_refused").replace("{{why}}", "")) && words.contains("not a MIDI file"), "{words}");
    assert!(state.music_page.own.is_empty(), "nothing was added");
    let _ = std::fs::remove_dir_all(&dir);
}

/// THE SHORTCUTS DO NOTHING WHILE A TEXT BOX HAS THE KEYBOARD (music-page.md section 16): with the
/// Written music search box focused, Space, N and 1 change nothing (the music stays on, the written
/// piece and the tab stay); with nothing focused, Space pauses the music and 1 opens Listen.
///
/// Seen RED 2026-11-20 with `keys::handle`'s text-box check removed: "Space while typing in the search
/// box leaves the music on".
#[test]
fn the_shortcuts_do_nothing_while_a_text_box_has_the_keyboard() {
    let (mut state, _) = music_state();
    state.settings.music.music_on = true;
    state.music_page.tab = Tab::Written;
    let search = egui::Id::new(super::written::SEARCH_ID);
    frames(&mut state, vec![vec![], vec![], vec![], key(Key::Space), key(Key::N), key(Key::Num1)], |i, ctx| {
        if i == 2 {
            ctx.memory_mut(|m| m.request_focus(search));
        }
    });
    assert!(state.settings.music.music_on, "Space while typing in the search box leaves the music on");
    assert_eq!(state.music_page.tab, Tab::Written, "1 while typing leaves the tab");
    frames(&mut state, vec![vec![], vec![], key(Key::Space), key(Key::Num1)], |_, _| {});
    assert!(!state.settings.music.music_on, "Space with nothing focused pauses the music");
    assert_eq!(state.music_page.tab, Tab::NowPlaying, "1 opens Now playing");
}

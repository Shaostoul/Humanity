//! THE MUSIC PAGE, RUNG 3a (docs/design/music-page.md section 15a and rung 2's leftovers): the sound
//! style chosen on Explore for all written music and on a piece's details for one piece, kept in the
//! person's settings and carried to what the score is asked to play; a program with no instrument of
//! ours said in words on the piece; the Swing and Intensity sliders changing what the score plays
//! (measured in its samples); A = 432 / 440 on the page. Each test draws the real page headlessly and
//! clicks what it drew.

use egui::{Pos2, Rect};
use humanity_core::i18n::t;
use humanity_music::{MusicContext, Resolved, SampleLibrary, ScoreEngine};

use super::tests::{click, music_state, text_rects, texts};
use super::Tab;
use crate::gui::GuiState;

/// Draw the Music page in a window of `size` for as many frames as `events` has; the last frame's
/// output, and the slider rail's width and the gap before a slider's own text, as the page's style
/// sets them.
fn frames_at(size: egui::Vec2, state: &mut GuiState, events: Vec<Vec<egui::Event>>) -> (egui::FullOutput, f32, f32) {
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    let mut out = None;
    for (i, ev) in events.into_iter().enumerate() {
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), time: Some(i as f64 / 60.0), events: ev, ..Default::default() };
        out = Some(ctx.run(input, |ctx| super::draw(ctx, &theme, state)));
    }
    let s = ctx.style();
    (out.unwrap(), s.spacing.slider_width, s.spacing.item_spacing.x)
}

const TALL: egui::Vec2 = egui::vec2(2560.0, 5000.0);

/// The first text drawn that begins with `prefix`, and where.
fn rect_starting(out: &egui::FullOutput, prefix: &str) -> Option<(String, Rect)> {
    text_rects(out).into_iter().find(|(w, _)| w.starts_with(prefix))
}

/// Electro swing, seed 5, in the calm mood, as the person's choice, on the Listen tab.
fn swing_chosen() -> (GuiState, std::sync::Arc<humanity_music::MusicLibrary>) {
    let (mut state, lib) = music_state();
    let s = &mut state.settings.music;
    s.game_chooses = false;
    s.genre = "electro_swing".into();
    s.mood = "mothership_calm".into();
    s.seed = 5;
    state.music_page.tab = Tab::NowPlaying;
    (state, lib)
}

/// `seconds` of what the person's settings choose, as the score would play it.
fn render_choice(state: &GuiState, lib: &humanity_music::MusicLibrary, seconds: f32) -> Vec<[f32; 2]> {
    let c = super::chosen(&state.settings.music, Some(lib)).expect("a choice");
    let g = lib.genre_index(&c.genre).unwrap() as u16;
    let mood = lib.mood_index(c.mood.as_deref().unwrap()).unwrap() as u16;
    let ctx = MusicContext { mood, genre_a: g, genre_b: g, intensity: c.intensity.unwrap_or(0.3), tune: c.tune, ..Default::default() };
    let mut e = ScoreEngine::new(Resolved::new(lib, &SampleLibrary::new()).unwrap(), 48000.0, c.seed, ctx);
    let mut buf = vec![[0.0f32; 2]; 128];
    let mut all = Vec::new();
    for _ in 0..(seconds * 48000.0 / 128.0) as usize {
        e.render(&mut buf);
        all.extend_from_slice(&buf);
    }
    all
}

fn rms(x: &[[f32; 2]]) -> f64 {
    (x.iter().map(|f| (f[0] as f64).powi(2) + (f[1] as f64).powi(2)).sum::<f64>() / (2 * x.len().max(1)) as f64).sqrt()
}

/// A click on the rail of the slider whose own text is `label`, `at` of the way along (0 the left end,
/// 1 the right): the rail ends a gap before its text and is `rail` wide.
fn on_rail(label: Rect, rail: f32, gap: f32, at: f32) -> Pos2 {
    let right = label.left() - gap - 2.0;
    let left = right - rail + 4.0;
    Pos2::new(left + (right - left) * at.clamp(0.0, 1.0), label.center().y)
}

/// A press on a slider's rail at `p`, let go a frame later (a slider senses a drag, which a press and
/// release in the same frame never starts).
fn press_on(p: Pos2) -> Vec<Vec<egui::Event>> {
    let down = vec![egui::Event::PointerMoved(p), egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }];
    let up = vec![egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }];
    vec![vec![], vec![], down, up, vec![]]
}

/// THE SWING AND INTENSITY SLIDERS CHANGE WHAT PLAYS (music-page.md rung 2's leftovers, built in rung
/// 3a): on the Listen tab, a click at the right end of the Swing slider (found by its own text) makes
/// the person's choice swing at 0.75 and the score render differently from before; a click at the
/// Intensity slider's left end makes it heard at 0, at its right end at 1, and the piece at 1 renders
/// louder than at 0 (its RMS, measured). The off-beats' place and the density are measured in the
/// crate (`swing_moves_the_off_beats_by_the_stated_amount`, `intensity_changes_the_measured_level_and_density`).
///
/// Seen RED 2026-11-21 with the Swing slider drawn without its own text (so the page offers no swing
/// a person can read): "the Tune card draws the Swing slider". (A first version clicked the rail with
/// a press and release in one frame, which never starts a slider's drag: "left: None".)
#[test]
fn the_swing_and_intensity_sliders_change_what_plays() {
    let (mut state, lib) = swing_chosen();
    let before = render_choice(&state, &lib, 6.0);
    let (out, rail, gap) = frames_at(TALL, &mut state, vec![vec![]; 3]);
    let swing_prefix = t("music_page.swing_value").split("{{").next().unwrap().to_string();
    let (_, swing) = rect_starting(&out, &swing_prefix).expect("the Tune card draws the Swing slider");
    frames_at(TALL, &mut state, press_on(on_rail(swing, rail, gap, 1.0)));
    assert_eq!(state.settings.music.tune.swing, Some(0.75), "a click at the slider's right end swings the off-beats at 0.75");
    let c = super::chosen(&state.settings.music, Some(&lib)).unwrap();
    assert_eq!(c.tune.swing, 0.75, "the score is asked for that swing");
    let after = render_choice(&state, &lib, 6.0);
    assert!(before.iter().zip(&after).any(|(a, b)| a != b), "and it plays differently from before");
    // The intensity: its slider's two ends.
    let (out, rail, gap) = frames_at(TALL, &mut state, vec![vec![]; 3]);
    let label = |out: &egui::FullOutput| {
        let rects = text_rects(out);
        let name = rects.iter().find(|(w, _)| w == t("music_page.intensity")).map(|(_, r)| *r).expect("the Tune card names the Intensity slider");
        // Its own text ("30%"), on the same line to the right of its name.
        rects.iter().filter(|(w, r)| w.ends_with('%') && (r.center().y - name.center().y).abs() < 6.0 && r.left() > name.right()).map(|(_, r)| *r).next().expect("the Intensity slider's own text")
    };
    let at = label(&out);
    frames_at(TALL, &mut state, press_on(on_rail(at, rail, gap, 0.0)));
    assert_eq!(super::chosen(&state.settings.music, Some(&lib)).unwrap().intensity, Some(0.0), "a click at the left end: heard at intensity 0");
    let calm = rms(&render_choice(&state, &lib, 10.0)[(2.0 * 48000.0) as usize..]);
    let (out, rail, gap) = frames_at(TALL, &mut state, vec![vec![]; 3]);
    let at = label(&out);
    frames_at(TALL, &mut state, press_on(on_rail(at, rail, gap, 1.0)));
    assert_eq!(super::chosen(&state.settings.music, Some(&lib)).unwrap().intensity, Some(1.0), "a click at the right end: heard at intensity 1");
    let busy = rms(&render_choice(&state, &lib, 10.0)[(2.0 * 48000.0) as usize..]);
    let db = 20.0 * (busy / calm.max(1e-12)).log10();
    assert!(db > 1.0, "intensity 1 plays louder than 0: {db:+.1} dB");
}

/// A = 432 OR 440 ON THE PAGE (music-page.md rung 2's leftovers, built in rung 3a): the Listen tab's
/// Tune card shows both tunings as radios with 432 chosen (the default); a click on 440 switches the
/// person's tuning, the one Settings > Music keeps, which every note the page plays follows.
///
/// Seen RED 2026-11-21 with the Tune card offering only 432: "the Tune card shows A = 440 Hz".
#[test]
fn the_tuning_is_shown_and_switched_on_the_page() {
    let (mut state, _lib) = swing_chosen();
    assert_eq!(state.settings.music.tuning, humanity_music::tuning::Tuning::A432);
    let (out, _, _) = frames_at(TALL, &mut state, vec![vec![]; 3]);
    let labels = super::tests::radio_labels(&out);
    assert!(labels.iter().any(|l| l == t("settings.music.tuning_432")), "the Tune card shows A = 432 Hz: {labels:?}");
    let r440 = text_rects(&out).into_iter().find(|(w, _)| w == t("settings.music.tuning_440")).map(|(_, r)| r).expect("the Tune card shows A = 440 Hz");
    frames_at(TALL, &mut state, vec![vec![], vec![], click(r440.left_center() - egui::vec2(10.0, 0.0))]);
    assert_eq!(state.settings.music.tuning, humanity_music::tuning::Tuning::A440, "a click on 440 switches the tuning");
    assert!(state.settings_dirty, "and the settings are saved");
}

/// A SOUND STYLE IS CHOSEN FOR ALL WRITTEN MUSIC AND FOR ONE PIECE, AND KEPT (music-page.md section
/// 15a): on Explore a click on the Solo piano card makes it the style for all written music; on Ode to
/// Joy's details the piece's own choice reads "Same as all written music (Solo piano)", and choosing
/// Retro FM there keeps that for this piece alone, which is what the score is asked to play it in
/// (`chosen`'s style), and the details then say what plays it: its melody on the retro FM lead.
///
/// Seen RED 2026-11-21 with Explore drawing As written alone (no card for a style file): "Explore draws
/// a card for the Solo piano style".
#[test]
fn a_sound_style_is_chosen_for_all_and_for_one_piece_and_kept() {
    let (mut state, lib) = music_state();
    let solo = lib.styles.iter().find(|s| s.id == "solo_piano").map(super::styles::style_name).unwrap();
    let retro = lib.styles.iter().find(|s| s.id == "retro_fm").map(super::styles::style_name).unwrap();
    state.music_page.tab = Tab::Explore;
    let (out, _, _) = frames_at(TALL, &mut state, vec![vec![]; 3]);
    let card = text_rects(&out).into_iter().find(|(w, _)| *w == solo).map(|(_, r)| r).expect("Explore draws a card for the Solo piano style");
    frames_at(TALL, &mut state, vec![vec![], vec![], click(card.center())]);
    assert_eq!(state.settings.music.style, "solo_piano", "the card's click chooses it for all written music");
    // Ode to Joy's details.
    state.music_page.tab = Tab::Written;
    state.music_page.written_selected = Some("ode_to_joy".into());
    let (out, _, _) = frames_at(TALL, &mut state, vec![vec![]; 3]);
    let same = t("music_page.style_same").replace("{{style}}", &solo);
    let combo = text_rects(&out).into_iter().find(|(w, _)| *w == same).map(|(_, r)| r).expect("the piece follows the style for all written music");
    // Open its list, then click Retro FM in it (the list draws above everything, so its entry is the
    // last Retro FM drawn).
    let (out, _, _) = frames_at(TALL, &mut state, vec![vec![], vec![], click(combo.center()), vec![]]);
    let pick = text_rects(&out).into_iter().filter(|(w, _)| *w == retro).map(|(_, r)| r).last().expect("the open list offers Retro FM");
    let (out, _, _) = frames_at(TALL, &mut state, vec![vec![], vec![], click(combo.center()), vec![], click(pick.center()), vec![]]);
    assert_eq!(state.settings.music.styles.get("ode_to_joy").map(String::as_str), Some("retro_fm"), "Retro FM kept for this piece");
    assert_eq!(state.settings.music.style, "solo_piano", "the style for all written music stays");
    state.settings.music.written = "ode_to_joy".into();
    assert_eq!(super::chosen(&state.settings.music, Some(&lib)).and_then(|c| c.style), Some("retro_fm".into()), "the score is asked to play it in Retro FM");
    let _ = out;
    let (out, _, _) = frames_at(TALL, &mut state, vec![vec![]; 3]);
    let lead = super::voice_words("retro_fm_lead");
    assert!(texts(&out).iter().any(|w| *w == lead), "the details say its melody plays on {lead:?}");
}

/// A small MIDI file: a violin (program 40), a bird tweet (123, no instrument of ours) and a bass (33).
fn test_midi() -> Vec<u8> {
    use humanity_music::smf::{Notated, NotatedPart, PNote, PPQ};
    let notes = |keys: &[u8]| keys.iter().enumerate().map(|(i, &k)| PNote { tick: i as u64 * PPQ as u64, len: PPQ as u64 / 2, key: k, vel: 100 }).collect::<Vec<_>>();
    let mut n = Notated { title: "Birdsong test".into(), ..Default::default() };
    n.parts.push(NotatedPart { name: "Violin".into(), program: Some(40), notes: notes(&[76, 79, 81, 79]), ..Default::default() });
    n.parts.push(NotatedPart { name: "Birds".into(), program: Some(123), notes: notes(&[84, 86, 88, 86]), ..Default::default() });
    n.parts.push(NotatedPart { name: "Bass".into(), program: Some(33), notes: notes(&[40, 43, 45, 43]), ..Default::default() });
    humanity_music::smf::write_smf(&n)
}

/// A PROGRAM WITH NO INSTRUMENT OF OURS IS SAID ON THE PAGE (music-page.md section 15a): an imported
/// file (a violin, a bird tweet and a bass) plays by its instrument numbers unless another style is
/// chosen; its details say which of our instruments plays each part, and in words that the bird
/// tweet (program 124 as people count) has no instrument of ours yet and plays on the piano.
///
/// Seen RED 2026-11-21 with the details drawing no style lines (rung 2's page): "the details say the
/// bird tweet has no instrument of ours: \"Part 2 (Melody) asks for program 124, Bird tweet, which has
/// no instrument of ours yet, so it plays on the piano.\"".
#[test]
fn a_program_with_no_instrument_of_ours_is_said_on_the_page() {
    let (mut state, _lib) = music_state();
    let imported = humanity_music::own::import_midi(&test_midi(), "birdsong.mid", &[]).unwrap();
    let mut entry = imported.entry;
    entry.folder = Some(std::env::temp_dir());
    let id = entry.id.clone();
    state.music_page.own = vec![entry.clone()];
    state.music_page.tab = Tab::Written;
    state.music_page.written_selected = Some(id.clone());
    let (out, _, _) = frames_at(TALL, &mut state, vec![vec![]; 3]);
    let all = texts(&out);
    let birds = entry.staves.iter().find(|s| s.program == Some(123)).unwrap().staff;
    let note = humanity_music::style::StyleNote::MissingProgram { staff: birds, program: 123, plays_on: "piano".into() };
    let said = super::styles::note_words(&entry, &note);
    assert!(said.contains(humanity_core::i18n::t_dyn("music.gm.program_123")), "the sentence names the bird tweet: {said}");
    assert!(all.iter().any(|w| *w == said), "the details say the bird tweet has no instrument of ours: {said:?}");
    let violin = super::voice_words("solo_violin");
    assert!(all.iter().any(|w| *w == violin), "and that the violin's part plays our {violin:?}");
}

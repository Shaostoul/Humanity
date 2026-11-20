//! ONE PLACE CHOOSES THE MUSIC (2026-11-14; the operator: "When I go to the settings it plays one
//! music and when I go to the music page it goes to another. We should have one place ideally or at
//! least have the synced so users don't get different music on different pages."). The Music page
//! chooses what plays and the choice lives in the person's own settings (`MusicSettings`), so the
//! score, the Music page and Settings > Music read one state. These tests draw the real pages
//! headlessly (no GPU) and read what they drew.

use std::sync::Arc;

use humanity_music::MusicLibrary;

use super::genre_label;
use crate::gui::GuiState;

/// Draw `page` for three frames in a window of `size` points, the last frame with `events`, and
/// return the last frame's output.
pub(crate) fn frame(size: egui::Vec2, events: Vec<egui::Event>, mut page: impl FnMut(&egui::Context)) -> egui::FullOutput {
    let ctx = egui::Context::default();
    crate::gui::install_fonts(&ctx);
    crate::gui::theme::load_theme().apply_to_egui(&ctx);
    let mut out = None;
    let mut events = Some(events);
    for i in 0..3 {
        let ev = if i == 2 { events.take().unwrap_or_default() } else { Vec::new() };
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)), time: Some(i as f64 / 60.0), events: ev, ..Default::default() };
        out = Some(ctx.run(input, |ctx| page(ctx)));
    }
    out.unwrap()
}

/// A click at `p`.
pub(crate) fn click(p: egui::Pos2) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(p),
        egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
        egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() },
    ]
}

/// Every text a frame drew.
pub(crate) fn texts(out: &egui::FullOutput) -> Vec<String> {
    fn walk(s: &egui::Shape, found: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => found.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    out.shapes.iter().for_each(|c| walk(&c.shape, &mut found));
    found
}

/// The labels of every radio button a frame drew: each text with a radio's ring just left of it on
/// its line (egui paints a radio as a circle, then its label a few points to the right). The genre picker
/// is radios, so another setting's level that shares a genre's name (Settings > Graphics' bounced-light
/// level "Ambient", a button in its own row) is not taken for one.
pub(crate) fn radio_labels(out: &egui::FullOutput) -> Vec<String> {
    fn walk(s: &egui::Shape, rings: &mut Vec<egui::Pos2>, words: &mut Vec<(String, egui::Rect)>) {
        match s {
            egui::Shape::Circle(c) => rings.push(c.center),
            egui::Shape::Text(t) => words.push((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2()))),
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, rings, words)),
            _ => {}
        }
    }
    let (mut rings, mut words) = (Vec::new(), Vec::new());
    out.shapes.iter().for_each(|c| walk(&c.shape, &mut rings, &mut words));
    words
        .into_iter()
        .filter(|(_, r)| rings.iter().any(|c| r.left() > c.x && r.left() - c.x < 30.0 && (r.center().y - c.y).abs() < r.height().max(8.0)))
        .map(|(w, _)| w)
        .collect()
}

/// A GuiState as the engine leaves it once the music's data is read: the library, the genres with
/// their words (built here from the data as engine/music.rs `genre_choices` does, stand-ins aside:
/// the GUI's tests do not reach up into the engine, the crate-layers lint), the tones and the "new
/// piece" choices (nothing playing).
pub(crate) fn music_state() -> (GuiState, Arc<MusicLibrary>) {
    let data = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let lib = Arc::new(MusicLibrary::load(&data).expect("the music data"));
    let mut state = GuiState::default();
    let r = &mut state.music_dev.readout;
    r.genres = lib.genres.iter().map(|g| crate::gui::pages::dev_music::GenreChoice { id: g.id.clone(), words: g.words.clone(), name: g.name.clone(), about_words: g.about_words.clone(), about: g.about.clone(), stand_ins: Vec::new() }).collect();
    r.tones = lib.tones.clone();
    r.piece_minutes = lib.piece_minutes.clone();
    r.lib = Some(lib.clone());
    r.device = true;
    // The favourites are the person's file: never read in a test.
    state.music_page.fav_loaded = true;
    (state, lib)
}

/// THERE IS EXACTLY ONE GENRE CONTROL, ON THE MUSIC PAGE: the whole Settings page (every section, in
/// a window tall enough to draw them all) draws no radio named for a genre, and the Music page draws
/// one radio for each genre. A genre named like a mood ("Mothership calm" is both) is held to the
/// Settings half only, since the Music page offers the moods as radios too.
///
/// Seen RED 2026-11-14 with Settings > Music as it was before this change (its file restored from
/// git: it drew its own genre radios beside the Music page's): "no Settings page draws a genre
/// picker: Settings shows \"Mothership calm\"". (A first version matched any drawn word, and Settings
/// > Graphics names a bounced-light level "Ambient", like the genre: so it reads radios.)
#[test]
fn there_is_one_genre_control_and_no_settings_page_draws_one() {
    let (mut state, lib) = music_state();
    let mut theme = crate::gui::theme::load_theme();
    let labels: Vec<String> = lib.genres.iter().map(|g| genre_label(&g.id, &g.words, &g.name)).collect();
    let moods: Vec<String> = lib.moods.iter().map(|m| super::mood_words(Some(&lib), &m.id)).collect();
    // Settings > Audio's voice devices are not asked of the machine in a test.
    state.audio_devices_loaded = true;
    let out = frame(egui::vec2(1600.0, 90000.0), Vec::new(), |ctx| crate::gui::pages::settings::draw(ctx, &mut theme, &mut state));
    assert!(texts(&out).iter().any(|x| x == humanity_core::i18n::t("settings.music.tuning")), "Settings > Music was drawn (its Tuning row)");
    let radios = radio_labels(&out);
    assert!(radios.iter().any(|x| x == humanity_core::i18n::t("settings.music.tuning_432")), "the radio finder finds Settings > Music's tuning radios");
    for l in &labels {
        assert!(!radios.contains(l), "no Settings page draws a genre picker: Settings shows {l:?}");
    }
    let out = frame(egui::vec2(1600.0, 6000.0), Vec::new(), |ctx| super::draw(ctx, &theme, &mut state));
    let radios = radio_labels(&out);
    for l in labels.iter().filter(|l| !moods.contains(l)) {
        assert_eq!(radios.iter().filter(|x| *x == l).count(), 1, "the Music page draws {l:?} once: the one genre control");
    }
    // A community genre with no words shows its plain name, else its id.
    assert_eq!(genre_label("chiptune", "", "Chiptune"), "Chiptune");
    assert_eq!(genre_label("chiptune", "", ""), "chiptune");
}

/// Draw Settings > Music alone, as the Settings page draws it.
fn settings_music_texts(state: &mut GuiState) -> Vec<String> {
    let theme = crate::gui::theme::load_theme();
    let out = frame(egui::vec2(1100.0, 3000.0), Vec::new(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| crate::gui::pages::settings_music::draw(ui, &theme, state, theme.nav_dev()));
    });
    texts(&out)
}

/// SETTINGS > MUSIC SAYS WHAT THE MUSIC PAGE SAYS IS PLAYING, in the same words, whoever chose it:
/// an electro swing piece (seed 4242) playing as the person's own choice, then a piece the game
/// chose. Both pages draw one sentence naming the genre and the seed, and it is the same sentence.
///
/// Seen RED 2026-11-14 with Settings > Music as it was before this change (its file restored from
/// git: no word of what plays): "Settings > Music says what the Music page says: \"Now playing:
/// Electro swing, seed 4242, your choice from the Music page.\"".
#[test]
fn settings_music_says_what_the_music_page_says() {
    let (mut state, lib) = music_state();
    let theme = crate::gui::theme::load_theme();
    let res = Arc::new(humanity_music::Resolved::new(&lib, &humanity_music::SampleLibrary::new()).expect("resolve"));
    let g = lib.genre_index("electro_swing").unwrap() as u16;
    let mood = lib.mood_index("mothership_calm").unwrap() as u16;
    let ctx_m = humanity_music::MusicContext { mood, genre_a: g, genre_b: g, ..Default::default() };
    let words = humanity_core::i18n::t("music.genre.electro_swing");
    for game_chooses in [false, true] {
        let m = &mut state.settings.music;
        m.genre = "electro_swing".into();
        m.game_chooses = game_chooses;
        m.mood = "mothership_calm".into();
        m.seed = 4242;
        let choice = super::chosen(m, Some(&lib));
        state.music_dev.readout.mood = "mothership_calm".into();
        state.music_dev.readout.playing = Some(crate::gui::pages::dev_music::Playing { choice, seed: 4242, ctx: ctx_m, res: res.clone(), lib: lib.clone() });
        let page = texts(&frame(egui::vec2(1600.0, 6000.0), Vec::new(), |ctx| super::draw(ctx, &theme, &mut state)));
        let says = page.iter().find(|x| x.contains(words) && x.contains("4242")).unwrap_or_else(|| panic!("the Music page says what plays (the game chooses: {game_chooses})")).clone();
        let settings = settings_music_texts(&mut state);
        assert!(settings.contains(&says), "Settings > Music says what the Music page says: {says:?}");
    }
    // The two ways are said differently: the person's choice is not the game's.
    let mut s = state.settings.music.clone();
    let mut r = state.music_dev.readout.clone();
    s.game_chooses = false;
    r.playing.as_mut().unwrap().choice = super::chosen(&s, Some(&lib));
    let yours = super::now_playing_words(&s, &r);
    s.game_chooses = true;
    r.playing.as_mut().unwrap().choice = None;
    let game = super::now_playing_words(&s, &r);
    assert_ne!(yours, game, "your choice and the game's are told apart");
}

/// Every text a frame drew, with where.
fn text_rects(out: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    fn walk(s: &egui::Shape, found: &mut Vec<(String, egui::Rect)>) {
        match s {
            egui::Shape::Text(t) => found.push((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2()))),
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    out.shapes.iter().for_each(|c| walk(&c.shape, &mut found));
    found
}

/// THE WRITTEN PIECES LIST PLAYS AND STOPS A PIECE (docs/design/score-player.md section 10): the Music
/// page lists the shipped Ode to Joy by its words with its composer; clicking its Play button (found
/// on its row in what the page drew) keeps the piece in the person's settings, which the score reads
/// as its choice, and says it starts at the next bar; with the engine reporting it playing the row
/// says so and Settings > Music says the same sentence as the page; Stop clears it and leaves who
/// chooses the rest as it was.
///
/// Seen RED 2026-11-20 with the list not drawn (`draw` skipping `written::written`): "its composer"
/// (the page drew no word of the piece).
#[test]
fn the_written_pieces_list_plays_and_stops_a_piece() {
    let (mut state, lib) = music_state();
    let theme = crate::gui::theme::load_theme();
    let title = humanity_core::i18n::t("music.score.ode_to_joy");
    let size = egui::vec2(1600.0, 6000.0);
    let button_on_row = |out: &egui::FullOutput, label: &str| -> egui::Pos2 {
        let drawn = text_rects(out);
        let row = drawn.iter().find(|(x, _)| x == title).unwrap_or_else(|| panic!("the Music page lists the written piece {title:?}")).1;
        drawn.iter().filter(|(x, r)| x == label && (r.center().y - row.center().y).abs() < 12.0).map(|(_, r)| r.center()).next().unwrap_or_else(|| panic!("a {label:?} button on the piece's row"))
    };
    let out = frame(size, Vec::new(), |ctx| super::draw(ctx, &theme, &mut state));
    assert!(texts(&out).iter().any(|x| x.contains(humanity_core::i18n::t("music.score.composer_beethoven"))), "its composer");
    let play = button_on_row(&out, humanity_core::i18n::t("music_page.written_play"));
    let out = frame(size, click(play), |ctx| super::draw(ctx, &theme, &mut state));
    assert_eq!(state.settings.music.written, "ode_to_joy", "Play keeps the piece in the person's settings");
    assert!(state.settings_dirty, "and the settings are saved");
    assert!(state.settings.music.game_chooses, "who chooses the rest is untouched");
    let choice = super::chosen(&state.settings.music, Some(&lib)).expect("a written piece is a choice");
    assert_eq!(choice.written.as_deref(), Some("ode_to_joy"), "the score reads it as its choice");
    let _ = out;
    let out = frame(size, Vec::new(), |ctx| super::draw(ctx, &theme, &mut state));
    assert!(texts(&out).iter().any(|x| x == humanity_core::i18n::t("music_page.written_starting")), "it starts at the next bar");
    // The engine reports it playing.
    let res = Arc::new(humanity_music::Resolved::new(&lib, &humanity_music::SampleLibrary::new()).expect("resolve"));
    state.music_dev.readout.written = Some("ode_to_joy".into());
    state.music_dev.readout.playing = Some(crate::gui::pages::dev_music::Playing { choice: Some(choice), seed: 1, ctx: humanity_music::MusicContext::default(), res, lib: lib.clone() });
    let out = frame(size, Vec::new(), |ctx| super::draw(ctx, &theme, &mut state));
    let page = texts(&out);
    assert!(page.iter().any(|x| x == humanity_core::i18n::t("music_page.written_playing")), "it says it plays");
    let says = page.iter().find(|x| x.contains(title) && x.contains(humanity_core::i18n::t("music_page.now_playing"))).expect("the page says what plays").clone();
    assert!(settings_music_texts(&mut state).contains(&says), "Settings > Music says what the Music page says: {says:?}");
    let stop = button_on_row(&out, humanity_core::i18n::t("music_page.written_stop"));
    frame(size, click(stop), |ctx| super::draw(ctx, &theme, &mut state));
    assert!(state.settings.music.written.is_empty(), "Stop clears it");
    assert!(super::chosen(&state.settings.music, Some(&lib)).is_none(), "and the game chooses again, as before");
}

//! THE MUSIC PAGE, VERSION 2 (docs/design/music-page.md, rung 1 of 2026-11-20): Like keeps the
//! playing piece in one click, the piano roll's length slider and its overview, the Written music
//! catalogue, and Explore's cards. Each test draws the real page headlessly (no GPU) and reads what
//! it DREW (texts, bar lines, the overview's window), never the value a control was set to.

use std::sync::Arc;

use humanity_core::i18n::{t, t_dyn};
use humanity_music::favourite::SavedTune;
use humanity_music::scores::ScoreEntry;
use humanity_music::sheet::Sheet;

use super::roll::{bars_shown, sheet_end, BAR_LINE_W, WINDOW_LINE_W};
use super::tests::{click, frame, music_state, text_rects, texts};
use super::written::{catalogue, era_words, WrittenQuery, WrittenSort};
use super::Tab;
use crate::gui::GuiState;

/// The operator's screen in points (2560 by 1440 at 100%).
const SCREEN: egui::Vec2 = egui::vec2(2560.0, 1440.0);

/// Every shape a frame drew, flattened.
fn shapes(out: &egui::FullOutput) -> Vec<egui::Shape> {
    fn walk(s: &egui::Shape, found: &mut Vec<egui::Shape>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, found)),
            other => found.push(other.clone()),
        }
    }
    let mut found = Vec::new();
    out.shapes.iter().for_each(|c| walk(&c.shape, &mut found));
    found
}

/// The roll's bar lines as drawn: the x of each vertical line of the bar-line width and colour.
fn bar_lines(out: &egui::FullOutput) -> Vec<f32> {
    let theme = crate::gui::theme::load_theme();
    let mut xs: Vec<f32> = shapes(out)
        .iter()
        .filter_map(|s| match s {
            egui::Shape::LineSegment { points, stroke } if (stroke.width - BAR_LINE_W).abs() < 1e-3 && stroke.color == theme.border() && (points[0].x - points[1].x).abs() < 0.01 => Some(points[0].x),
            _ => None,
        })
        .collect();
    xs.sort_by(f32::total_cmp);
    xs
}

/// The number drawn just right of a bar line (the bar's number, counted from 1).
fn bar_number_at(out: &egui::FullOutput, x: f32) -> Option<usize> {
    text_rects(out).into_iter().filter(|(w, r)| r.left() > x && r.left() - x < 8.0 && !w.is_empty() && w.chars().all(|c| c.is_ascii_digit())).find_map(|(w, _)| w.parse().ok())
}

/// The overview's background and its window, as drawn.
fn overview_rects(out: &egui::FullOutput) -> (egui::Rect, egui::Rect) {
    let theme = crate::gui::theme::load_theme();
    let all = shapes(out);
    let window = all
        .iter()
        .find_map(|s| match s {
            egui::Shape::Rect(r) if (r.stroke.width - WINDOW_LINE_W).abs() < 1e-3 && r.stroke.color == theme.accent() => Some(r.rect),
            _ => None,
        })
        .expect("the overview draws its window");
    let back = all
        .iter()
        .find_map(|s| match s {
            egui::Shape::Rect(r) if r.fill == theme.bg_tertiary() && r.rect.expand(1.5).contains_rect(window) => Some(r.rect),
            _ => None,
        })
        .expect("the window sits in the overview");
    (back, window)
}

/// The music state with Für Elise playing, `now` seconds in, and its sheet as the page writes it.
fn fur_elise_playing(now: f64) -> (GuiState, Sheet) {
    let (mut state, lib) = music_state();
    let data = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let piece = Arc::new(humanity_music::scores::load_piece(&data, lib.score("fur_elise").expect("listed")).expect("it reads"));
    let sheet = Sheet::from_timeline(&piece.timeline, &piece.staff_roles());
    state.settings.music.written = "fur_elise".into();
    state.music_dev.readout.written = Some("fur_elise".into());
    state.music_dev.readout.written_piece = Some(piece);
    state.music_dev.readout.score = Some(humanity_music::MusicStatus { seconds: now, ..Default::default() });
    (state, sheet)
}

/// LIKE KEEPS THE PLAYING PIECE, WITH ITS CHANGES, IN ONE CLICK (the operator, 2026-11-20: "We should
/// add a button to save a liked seed"): an electro swing piece (seed 4242) playing as the person's
/// choice, with the bass 3 dB down, the texture muted and the key two steps up. One click on the
/// strip's Like and the favourites hold that piece: its genre, mood, seed and every change, under a
/// name made for it; the button then says Liked, and a second click keeps one favourite and opens Your
/// music with it marked. While the game chooses, Like keeps what plays now (its seed, the game's mood).
///
/// Seen RED 2026-11-20 with `strip::like` returning at once: "one click keeps one favourite (left: 0,
/// right: 1)".
#[test]
fn like_keeps_the_playing_piece_and_its_changes_in_one_click() {
    let (mut state, lib) = music_state();
    let theme = crate::gui::theme::load_theme();
    let g = lib.genre_index("electro_swing").unwrap();
    let mut tune = humanity_music::tune::Tune::default();
    tune.level_db[1] = -3.0;
    tune.mute = 1 << 7;
    tune.transpose = 2;
    let m = &mut state.settings.music;
    m.genre = "electro_swing".into();
    m.game_chooses = false;
    m.mood = "mothership_calm".into();
    m.seed = 4242;
    m.tune = SavedTune::from_tune(&tune, &lib, g);
    let like_at = |out: &egui::FullOutput, words: &str| text_rects(out).into_iter().find(|(w, _)| w == words).map(|(_, r)| r.center()).unwrap_or_else(|| panic!("the strip shows {words:?}"));
    let out = frame(SCREEN, Vec::new(), |ctx| super::draw(ctx, &theme, &mut state));
    frame(SCREEN, click(like_at(&out, t("music_page.like"))), |ctx| super::draw(ctx, &theme, &mut state));
    let favs = state.music_page.favourites.favourites.clone();
    assert_eq!(favs.len(), 1, "one click keeps one favourite");
    let f = &favs[0];
    assert_eq!((f.genre.as_str(), f.mood.as_str(), f.seed), ("electro_swing", "mothership_calm", 4242), "the piece playing");
    let kept = f.tune.to_tune(&lib, g);
    assert_eq!((kept.level_db[1], kept.mute, kept.transpose), (-3.0, 1 << 7, 2), "with every change made to it");
    assert_eq!(f.name, t("music_page.liked_name").replace("{{genre}}", t("music.genre.electro_swing")).replace("{{seed}}", "4242"), "under a name made for it");
    // Already kept: the button says so, and a second click opens Your music with it marked.
    let out = frame(SCREEN, Vec::new(), |ctx| super::draw(ctx, &theme, &mut state));
    frame(SCREEN, click(like_at(&out, t("music_page.liked"))), |ctx| super::draw(ctx, &theme, &mut state));
    assert_eq!(state.music_page.favourites.favourites.len(), 1, "a second click keeps it once (and never removes it)");
    assert_eq!((state.music_page.tab, state.music_page.marked), (Tab::Yours, Some(f.id)), "it opens Your music with the favourite marked");
    let out = frame(SCREEN, Vec::new(), |ctx| super::draw(ctx, &theme, &mut state));
    assert!(texts(&out).contains(&f.name), "Your music lists it by its name");
    // While the game chooses, the piece that plays now.
    let res = Arc::new(humanity_music::Resolved::new(&lib, &humanity_music::SampleLibrary::new()).expect("resolve"));
    let lofi = lib.genre_index("lofi").unwrap() as u16;
    state.settings.music.genre = "lofi".into();
    state.settings.music.game_chooses = true;
    state.music_dev.readout.mood = "night_watch".into();
    state.music_dev.readout.playing = Some(crate::gui::pages::dev_music::Playing { choice: None, seed: 777, ctx: humanity_music::MusicContext { genre_a: lofi, genre_b: lofi, ..Default::default() }, res, lib: lib.clone() });
    state.music_page.tab = Tab::Listen;
    let out = frame(SCREEN, Vec::new(), |ctx| super::draw(ctx, &theme, &mut state));
    frame(SCREEN, click(like_at(&out, t("music_page.like"))), |ctx| super::draw(ctx, &theme, &mut state));
    let last = state.music_page.favourites.favourites.last().cloned().expect("a second favourite");
    assert_eq!((last.genre.as_str(), last.mood.as_str(), last.seed), ("lofi", "night_watch", 777), "the game's piece as it plays");
}

/// THE LENGTH SLIDER CHANGES HOW MANY BARS THE ROLL DRAWS (the operator, 2026-11-20: "could we draw the
/// whole song or variable? Like a slider or something choosing visualized length"): with Für Elise
/// playing, the roll draws 2, 4, 8 and 16 bars and then the whole piece, counted from the bar lines it
/// DREW (one at the start of each bar), and the first bar drawn carries the number 1. The slider's
/// own steps run from 2 bars to the whole piece.
///
/// Seen RED 2026-11-20 with the roll drawing four bars whatever was chosen (`let n = 4.min(total)` in
/// `roll::panel`): "asked for 2 bars (0: the whole piece), the roll drew 4 (left: 4, right: 2)".
#[test]
fn the_length_slider_changes_how_many_bars_are_drawn() {
    let (mut state, sheet) = fur_elise_playing(0.0);
    let theme = crate::gui::theme::load_theme();
    let total = sheet.bars.len();
    assert!(total > 32, "Für Elise has more than 32 bars ({total})");
    assert_eq!(super::roll::length_steps(total).first(), Some(&2));
    assert_eq!(super::roll::length_steps(total).last(), Some(&0), "the last step is the whole piece");
    for asked in [2usize, 4, 8, 16, 0] {
        state.music_page.roll_bars = asked;
        let out = frame(SCREEN, Vec::new(), |ctx| super::draw(ctx, &theme, &mut state));
        let lines = bar_lines(&out);
        let want = bars_shown(asked, total);
        assert_eq!(lines.len(), want, "asked for {asked} bars (0: the whole piece), the roll drew {}", lines.len());
        assert_eq!(bar_number_at(&out, lines[0]), Some(1), "the first bar drawn is bar 1");
    }
}

/// THE OVERVIEW'S WINDOW MARKS THE BARS THE ROLL DRAWS: with Follow the music off and the window put at
/// bar 11, four bars wide, the roll draws bars 11 to 14 and the overview's window spans the whole
/// piece's time from bar 11's start to bar 14's end (to a pixel); following the music half-way through,
/// the playhead is inside the window; and a click three quarters along the overview moves the roll
/// there and lets go of following.
///
/// Seen RED 2026-11-20 with the window drawn one bar late (`sheet.bar_span(first + 1)`): "the window
/// starts at 210.71199, bar 11 starts at 190.6".
#[test]
fn the_overview_window_matches_the_visible_bars() {
    let (mut state, sheet) = fur_elise_playing(0.0);
    let theme = crate::gui::theme::load_theme();
    let end = sheet_end(&sheet);
    state.music_page.roll_bars = 4;
    state.music_page.roll_follow = false;
    state.music_page.roll_first = 10;
    let out = frame(SCREEN, Vec::new(), |ctx| super::draw(ctx, &theme, &mut state));
    let lines = bar_lines(&out);
    assert_eq!(lines.len(), 4, "four bars drawn");
    assert_eq!(bar_number_at(&out, lines[0]), Some(11), "from bar 11");
    let (back, win) = overview_rects(&out);
    let x_of = |s: f64| back.left() + (s / end) as f32 * back.width();
    let (a, _) = sheet.bar_span(10).unwrap();
    let (_, b) = sheet.bar_span(13).unwrap();
    assert!((win.left() - x_of(a)).abs() <= 1.0, "the window starts at {}, bar 11 starts at {}", win.left(), x_of(a));
    assert!((win.right() - x_of(b)).abs() <= 1.0, "the window ends at {}, bar 14 ends at {}", win.right(), x_of(b));
    // Following the music half-way through: the playhead in the window, the bars drawn around it.
    let half = end * 0.5;
    state.music_dev.readout.score = Some(humanity_music::MusicStatus { seconds: half, ..Default::default() });
    state.music_page.roll_follow = true;
    let out = frame(SCREEN, Vec::new(), |ctx| super::draw(ctx, &theme, &mut state));
    let (back, win) = overview_rects(&out);
    let x_of = |s: f64| back.left() + (s / end) as f32 * back.width();
    assert!(win.left() <= x_of(half) && x_of(half) <= win.right(), "the playhead at {} is inside the window {win:?}", x_of(half));
    let first = bar_number_at(&out, bar_lines(&out)[0]).expect("numbered");
    let playing = sheet.bar_at(half).unwrap() + 1;
    assert!(first <= playing && playing < first + 4, "bar {playing} plays and the roll draws {first} onwards");
    // A click three quarters along the overview: the roll goes there and stops following.
    let at = egui::pos2(back.left() + back.width() * 0.75, back.center().y);
    frame(SCREEN, click(at), |ctx| super::draw(ctx, &theme, &mut state));
    assert!(!state.music_page.roll_follow, "a click on the overview lets go of following");
    let out = frame(SCREEN, Vec::new(), |ctx| super::draw(ctx, &theme, &mut state));
    let first = bar_number_at(&out, bar_lines(&out)[0]).expect("numbered");
    let clicked = sheet.bar_at(end * 0.75).unwrap() + 1;
    assert!(first <= clicked && clicked < first + 4, "the click at bar {clicked} shows bars from {first}");
    // Rung 2: a written piece also plays from the bar the click landed on.
    assert_eq!(state.music_page.seek, (1, (clicked - 1) as u32), "a click on a written piece's overview seeks to its bar");
}

/// The row titles the Written music tab drew, top to bottom (the list is the left part of the page;
/// the details' heading is to its right).
fn drawn_rows(out: &egui::FullOutput, titles: &[String]) -> Vec<String> {
    let mut rows: Vec<(String, egui::Rect)> = text_rects(out).into_iter().filter(|(w, r)| titles.contains(w) && r.left() < SCREEN.x * 0.5).collect();
    rows.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()));
    rows.into_iter().map(|r| r.0).collect()
}

/// THE WRITTEN LIST FILTERS AND SORTS FROM DATA (the operator, 2026-11-20: "a searchable list for
/// infinite songs, including the little details, like the origin/source"): from the shipped index,
/// a search for "beethoven" keeps his three pieces; "Samelis" (a typesetter) the two he typeset; two
/// words both; the composer filter Grieg and the era filter Romantic keep the Mountain King; sorted by
/// composer Beethoven's come first, by length the shortest first. A community entry added to the data
/// (Clara Schumann's Romance, no code) is listed among the Romantic pieces and sorted by year. The
/// page draws the rows in the catalogue's order.
///
/// Seen RED 2026-11-20 with `catalogue` keeping every row in the index's order (no search, filter or
/// sort): search "beethoven", left: [ode_to_joy, fur_elise, mountain_king, beethoven_symphony5_1],
/// right: [fur_elise, ode_to_joy, beethoven_symphony5_1].
#[test]
fn the_written_list_filters_and_sorts_from_data() {
    let (mut state, lib) = music_state();
    let theme = crate::gui::theme::load_theme();
    let ids = |entries: &[ScoreEntry], q: &WrittenQuery| -> Vec<String> { catalogue(entries, q).into_iter().map(|i| entries[i].id.clone()).collect() };
    let q = |text: &str, sort: WrittenSort, composer: &str, era: &str| WrittenQuery { text: text.into(), sort, composer: composer.into(), era: era.into() };
    let s = &lib.scores;
    let grieg = t("music.score.composer_grieg").to_string();
    let romantic = t("music.era.romantic").to_string();
    assert_eq!(ids(s, &q("beethoven", WrittenSort::Title, "", "")), ["fur_elise", "ode_to_joy", "beethoven_symphony5_1"], "search \"beethoven\"");
    assert_eq!(ids(s, &q("Samelis", WrittenSort::Title, "", "")), ["fur_elise", "beethoven_symphony5_1"], "search a typesetter");
    assert_eq!(ids(s, &q("beethoven symphony", WrittenSort::Title, "", "")), ["beethoven_symphony5_1"], "every word typed");
    assert_eq!(ids(s, &q("", WrittenSort::Title, &grieg, "")), ["mountain_king"], "the composer filter");
    assert_eq!(ids(s, &q("", WrittenSort::Title, "", &romantic)), ["mountain_king"], "the era filter");
    assert_eq!(ids(s, &q("", WrittenSort::Composer, "", "")).last().map(String::as_str), Some("mountain_king"), "by composer: Beethoven, then Grieg");
    assert_eq!(ids(s, &q("", WrittenSort::Length, "", "")), ["ode_to_joy", "mountain_king", "fur_elise", "beethoven_symphony5_1"], "by length, shortest first");
    // A community entry: a file and its index line, no code.
    let romance: ScoreEntry = ron::from_str(r#"(id: "schumann_romance", file: "schumann_romance.mei", name: "Romance in A minor", composer: "Clara Schumann", era: "Romantic", year: 1853, length_s: 200.0, staves: [(staff: 1, instrument: "electric_piano", part: Some("piano"))])"#).expect("a community index line reads");
    let mut more = s.clone();
    more.push(romance);
    assert_eq!(era_words(more.last().unwrap()), "Romantic");
    assert_eq!(ids(&more, &q("", WrittenSort::Year, "", &romantic)), ["schumann_romance", "mountain_king"], "a new entry is listed and sorted by year (1853 before 1875)");
    // The page draws the rows in that order.
    let titles: Vec<String> = s.iter().map(|e| super::score_words(e).0).collect();
    state.music_page.tab = Tab::Written;
    state.music_page.written_query = q("", WrittenSort::Length, "", "");
    let out = frame(SCREEN, Vec::new(), |ctx| super::draw(ctx, &theme, &mut state));
    let want: Vec<String> = ids(s, &state.music_page.written_query).iter().map(|id| super::score_words(lib.score(id).unwrap()).0).collect();
    assert_eq!(drawn_rows(&out, &titles), want, "the page draws the rows by length");
    state.music_page.written_query = q("beethoven", WrittenSort::Composer, "", "");
    let out = frame(SCREEN, Vec::new(), |ctx| super::draw(ctx, &theme, &mut state));
    assert!(!drawn_rows(&out, &titles).contains(&super::score_words(lib.score("mountain_king").unwrap()).0), "the search hides the Mountain King");
    // The details say where the chosen piece came from.
    state.music_page.written_query = WrittenQuery::default();
    state.music_page.written_selected = Some("fur_elise".into());
    let out = frame(SCREEN, Vec::new(), |ctx| super::draw(ctx, &theme, &mut state));
    let drawn = texts(&out);
    let e = lib.score("fur_elise").unwrap();
    for detail in [e.edition.as_str(), e.source.as_str(), e.typesetter.as_str(), e.licence.as_str(), t_dyn(&e.tempo_words)] {
        assert!(drawn.iter().any(|x| x == detail), "the details show {detail:?}");
    }
}

/// EVERY GENRE AND MOOD FILE GETS A CARD WITH ITS PLAIN SENTENCE (the operator, 2026-11-20: "What
/// each style means and mood is kind of confusing"): on Explore, each genre's card shows its name and
/// the one plain sentence its data file names, and each mood's card where it plays. A community genre
/// read from its own file (here a copy of lo-fi named Chiptune, with plain words and no keys) gets its
/// card and its sentence with no code change.
///
/// Seen RED 2026-11-20 with the cards showing each genre's "about" in place of its plain sentence:
/// "the Mothership calm card shows its plain sentence \"The ship's own quiet music: slow and spacious,
/// for relaxing, reading or drifting off.\"".
#[test]
fn every_genre_and_mood_file_gets_a_card_with_its_sentence() {
    let (mut state, lib) = music_state();
    let theme = crate::gui::theme::load_theme();
    state.music_page.tab = Tab::Explore;
    let out = frame(SCREEN, Vec::new(), |ctx| super::draw(ctx, &theme, &mut state));
    let drawn = texts(&out);
    for g in &lib.genres {
        let name = super::genre_label(&g.id, &g.words, &g.name);
        let plain = t_dyn(&g.plain_words);
        assert!(drawn.contains(&name), "a card for {name}");
        assert!(drawn.iter().any(|x| x == plain), "the {name} card shows its plain sentence {plain:?}");
    }
    for m in &lib.moods {
        let place = t_dyn(&m.where_words);
        assert!(drawn.iter().any(|x| x == place), "the {} card says where it plays: {place:?}", m.id);
    }
    // A community genre file: read from its words, added to the music's data, no code.
    let data = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let text = std::fs::read_to_string(data.join("music/genres/lofi.ron")).unwrap();
    let text = text.replace("id: \"lofi\"", "id: \"chiptune\"").replace("words: \"music.genre.lofi\"", "name: \"Chiptune\"").replace("plain_words: \"music.genre.lofi_plain\"", "plain: \"Bleeps and bloops from old game consoles.\"");
    let chip: humanity_music::genre::Genre = ron::from_str(&text).expect("a community genre file reads");
    let mut more = (*lib).clone();
    more.genres.push(chip);
    state.music_dev.readout.lib = Some(Arc::new(more));
    let out = frame(SCREEN, Vec::new(), |ctx| super::draw(ctx, &theme, &mut state));
    let drawn = texts(&out);
    assert!(drawn.iter().any(|x| x == "Chiptune"), "the new genre has a card");
    assert!(drawn.iter().any(|x| x == "Bleeps and bloops from old game consoles."), "with its own sentence");
}

/// THE PIECES HEARD: each piece once, newest first; a piece heard again moves to the top; a level
/// moved is the same piece; a written piece is listed by its id. RED 2026-11-20 with `note_heard`
/// keeping nothing it had (its same-piece check and its retain broken): "one entry a piece (left: 1,
/// right: 2)".
#[test]
fn the_pieces_heard_list_each_piece_once_newest_first() {
    let (mut state, lib) = music_state();
    let res = Arc::new(humanity_music::Resolved::new(&lib, &humanity_music::SampleLibrary::new()).expect("resolve"));
    let g = lib.genre_index("lofi").unwrap() as u16;
    let play = |state: &mut GuiState, seed: u64, level: f32| {
        let mut ctx = humanity_music::MusicContext { genre_a: g, genre_b: g, ..Default::default() };
        ctx.tune.level_db[0] = level;
        state.music_dev.readout.playing = Some(crate::gui::pages::dev_music::Playing { choice: None, seed, ctx, res: res.clone(), lib: lib.clone() });
        let (s, r) = (state.settings.music.clone(), state.music_dev.readout.clone());
        state.music_page.note_heard(&s, &r);
    };
    play(&mut state, 5, 0.0);
    play(&mut state, 5, 0.0);
    play(&mut state, 6, 0.0);
    assert_eq!(state.music_page.heard.len(), 2, "one entry a piece");
    play(&mut state, 6, -6.0);
    assert_eq!(state.music_page.heard.len(), 2, "a level moved is the same piece");
    play(&mut state, 5, 0.0);
    assert_eq!(state.music_page.heard.iter().map(|h| h.seed).collect::<Vec<_>>(), [5, 6], "heard again, it moves to the top");
    assert_eq!(state.music_page.heard[0].genre, "lofi");
    state.music_dev.readout.written = Some("ode_to_joy".into());
    let (s, r) = (state.settings.music.clone(), state.music_dev.readout.clone());
    state.music_page.note_heard(&s, &r);
    assert_eq!(state.music_page.heard[0].written.as_deref(), Some("ode_to_joy"), "a written piece is listed");
}

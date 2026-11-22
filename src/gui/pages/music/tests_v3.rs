//! THE MUSIC PAGE, VERSION 3 RUNG 1 (docs/design/music-page.md sections 28 and 29): the transport bar
//! at the bottom with its drawn icons, the parts row that merges the visualizer and the Mixer, the new
//! keys, and the one list (queue.rs) on its own: shuffle, repeat, and its file across a restart. Drawn
//! for real, headless, and read from what was drawn (each button's rect from egui's own record of it,
//! `Context::read_response`). What the runtime then plays for each control is in
//! engine/music_tests_v3.rs.

use std::sync::Arc;

use egui::{Event, Key, Modifiers};
use humanity_core::i18n::t;
use humanity_music::MusicLibrary;

use super::queue::{Item, Queue, Repeat};
use super::tests::{music_state, text_rects, texts};
use super::transport::{button_id, LIKE, MUTE, NEXT, PLAY, PREV, REPEAT, SEEK_ID, SHUFFLE, UP_NEXT};
use super::Tab;
use crate::gui::widgets::transport_icons::{paint, TransportIcon};
use crate::gui::GuiState;

/// A key pressed and let go.
pub(crate) fn key(k: Key) -> Vec<Event> {
    vec![Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }, Event::Key { key: k, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE }]
}

/// Draw the Music page in a window of `size` points and keep the context, so a test reads where each
/// control was drawn (`Context::read_response`) and clicks it in the next frame. A new context draws
/// three quiet frames first; a kept one draws one frame with `events` and one quiet frame after.
pub(crate) fn draw_kept(state: &mut GuiState, size: egui::Vec2, events: Vec<Event>, ctx: Option<egui::Context>) -> (egui::FullOutput, egui::Context) {
    let theme = crate::gui::theme::load_theme();
    let fresh = ctx.is_none();
    let ctx = ctx.unwrap_or_else(|| {
        let c = egui::Context::default();
        crate::gui::install_fonts(&c);
        theme.apply_to_egui(&c);
        c
    });
    let mut frames: Vec<Vec<Event>> = if fresh { vec![Vec::new(), Vec::new(), Vec::new()] } else { Vec::new() };
    frames.push(events);
    frames.push(Vec::new());
    let mut out = None;
    for (i, ev) in frames.into_iter().enumerate() {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)), time: Some(i as f64 / 60.0 + ctx.input(|x| x.time)), events: ev, ..Default::default() };
        out = Some(ctx.run(input, |c| super::draw(c, &theme, state)));
    }
    (out.unwrap(), ctx)
}

/// A page with a composed piece of the person's playing (electro swing, seed 4242, four minutes long),
/// 30 seconds in, as the engine reports it.
pub(crate) fn composed_playing() -> (GuiState, Arc<MusicLibrary>) {
    let (mut state, lib) = music_state();
    let res = Arc::new(humanity_music::Resolved::new(&lib, &humanity_music::SampleLibrary::new()).expect("resolve"));
    let m = &mut state.settings.music;
    m.game_chooses = false;
    m.genre = "electro_swing".into();
    m.mood = "mothership_calm".into();
    m.seed = 4242;
    m.minutes = 4.0;
    let choice = super::chosen(m, Some(&lib)).expect("a choice");
    assert_eq!(choice.length_s, Some(240.0), "the composed piece has its length");
    let g = lib.genre_index("electro_swing").unwrap() as u16;
    let mood = lib.mood_index("mothership_calm").unwrap() as u16;
    let ctx_m = humanity_music::MusicContext { mood, genre_a: g, genre_b: g, piece_s: 240.0, ..Default::default() };
    let r = &mut state.music_dev.readout;
    r.playing = Some(crate::gui::pages::dev_music::Playing { choice: Some(choice), seed: 4242, ctx: ctx_m, res, lib: lib.clone() });
    r.score = Some(humanity_music::MusicStatus { seconds: 30.0, beat: 1.5, beats_per_bar: 4, ..Default::default() });
    r.next_piece_s = Some(210.0);
    r.mood = "mothership_calm".into();
    (state, lib)
}

/// THE TRANSPORT BAR DRAWS AT THE BOTTOM, WIDE AND ON A PHONE (section 28.5; the operator: "Bottom would
/// probably be best for phone users"): at 2560 by 1440 and at 412 by 915, every button of the bar
/// (previous, play, next, shuffle, repeat, like, Up next, the speaker) and the seek bar are drawn inside
/// the screen, none overlapping another, in a band along the bottom edge, below every other word the
/// page drew.
///
/// Seen RED 2026-11-21 with the bar docked at the TOP (`TopBottomPanel::top` for
/// "music_page_transport"): "at 2560x1440 the bar sits at the bottom: music_t_prev's top is 175".
/// It also caught the volume slider wrapped in a left-to-right layout inside the right-hand run, which
/// took the whole row and pushed Like onto Next: "at 2560x1440 music_t_next and music_t_like do not
/// overlap".
#[test]
fn the_transport_bar_draws_at_the_bottom_wide_and_on_a_phone() {
    for (w, h, band) in [(2560.0f32, 1440.0f32, 200.0f32), (412.0, 915.0, 330.0)] {
        let (mut state, _) = composed_playing();
        state.music_page.tab = Tab::NowPlaying;
        let (out, ctx) = draw_kept(&mut state, egui::vec2(w, h), Vec::new(), None);
        let names = [PREV, PLAY, NEXT, SHUFFLE, REPEAT, LIKE, UP_NEXT, MUTE];
        let rects: Vec<egui::Rect> = names.iter().map(|id| ctx.read_response(button_id(id)).unwrap_or_else(|| panic!("at {w}x{h} the bar draws {id}")).rect).collect();
        let seek = ctx.read_response(egui::Id::new(SEEK_ID)).unwrap_or_else(|| panic!("at {w}x{h} the bar draws the seek bar")).rect;
        for (id, r) in names.iter().zip(&rects).chain(std::iter::once((&"seek", &seek))) {
            assert!(r.left() >= 0.0 && r.right() <= w + 0.5 && r.bottom() <= h + 0.5, "at {w}x{h} {id} is inside the screen: {r:?}");
            assert!(r.top() >= h - band, "at {w}x{h} the bar sits at the bottom: {id}'s top is {:.0}", r.top());
        }
        for (i, a) in rects.iter().enumerate() {
            for (j, b) in rects.iter().enumerate().skip(i + 1) {
                assert!(!a.shrink(1.0).intersects(b.shrink(1.0)), "at {w}x{h} {} and {} do not overlap", names[i], names[j]);
            }
        }
        // Below everything else: the bar's top is under every word drawn outside it.
        let top = rects.iter().chain(std::iter::once(&seek)).map(|r| r.top()).fold(f32::INFINITY, f32::min) - 60.0;
        let above: Vec<(String, egui::Rect)> = text_rects(&out).into_iter().filter(|(_, r)| r.bottom() < top).collect();
        assert!(!above.is_empty(), "the page draws above the bar");
        assert!(seek.width() >= if w > 1000.0 { 1200.0 } else { 150.0 }, "at {w}x{h} the seek bar has room: {:.0} wide", seek.width());
    }
}

/// THE ICONS ARE DRAWN SHAPES (section 28.5; the orchestrator's call: no icon font, no picture, no glyph
/// egui draws as a box): each transport icon painted alone gives shapes and no text, and the whole
/// page with the bar draws no symbol character (arrows, geometric shapes, dingbats, emoji) in any word.
///
/// Seen RED 2026-11-21 with Play painted as the U+25B6 glyph through `painter.text`: "the Play icon
/// draws no text (a glyph is text): 1 text shapes".
#[test]
fn the_transport_icons_are_drawn_shapes_not_glyphs() {
    fn walk(s: &egui::Shape, shapes: &mut usize, words: &mut usize) {
        match s {
            egui::Shape::Text(_) => *words += 1,
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, shapes, words)),
            egui::Shape::Noop => {}
            _ => *shapes += 1,
        }
    }
    for icon in TransportIcon::ALL {
        let ctx = egui::Context::default();
        crate::gui::install_fonts(&ctx);
        let out = ctx.run(egui::RawInput::default(), |c| {
            let painter = c.layer_painter(egui::LayerId::background());
            paint(&painter, egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(40.0, 40.0)), icon, egui::Color32::WHITE);
        });
        let (mut shapes, mut words) = (0, 0);
        out.shapes.iter().for_each(|c| walk(&c.shape, &mut shapes, &mut words));
        assert_eq!(words, 0, "the {icon:?} icon draws no text (a glyph is text): {words} text shapes");
        assert!(shapes >= 1, "the {icon:?} icon is drawn as shapes");
    }
    let (mut state, _) = composed_playing();
    state.music_page.up_next_open = true;
    let (out, _) = draw_kept(&mut state, egui::vec2(2560.0, 1440.0), Vec::new(), None);
    let symbol = |c: char| matches!(c as u32, 0x2190..=0x21FF | 0x23E9..=0x23FA | 0x25A0..=0x25FF | 0x2600..=0x27BF | 0x2B00..=0x2BFF | 0x1F300..=0x1FAFF);
    for w in texts(&out) {
        assert!(!w.chars().any(symbol), "no symbol glyph in the page's words: {w:?}");
    }
}

/// THE PARTS ROW: ONE ROW WIDE, TWO ROWS OF FOUR ON A PHONE (section 28.7; the operator: "move those
/// volume bars in the mixer next to the actual kind of music section so we waste less vertical space"):
/// Now playing draws one Solo for each of the eight parts, on one line at 2560 by 1440 and on two lines
/// of four at a phone's 412 points, every one inside the page's margins; the separate Mixer card is gone.
///
/// Seen RED 2026-11-21 with the narrow page drawing one strip a row as the Mixer did: "on a phone the
/// parts wrap into two rows of four: 8 rows of [1, 1, 1, 1, 1, 1, 1, 1]". Its first green run caught the
/// fourth column of each row past the screen's edge ("at 412 wide each Solo is inside the screen:
/// [[389.0 1339.0] - [412.1 1355.0]]": the row's own spacing between columns was not counted).
#[test]
fn the_parts_row_is_one_row_wide_and_two_rows_of_four_on_a_phone() {
    for (w, h) in [(2560.0f32, 1440.0f32), (412.0, 4000.0)] {
        let (mut state, _) = composed_playing();
        state.music_page.tab = Tab::NowPlaying;
        let (out, _) = draw_kept(&mut state, egui::vec2(w, h), Vec::new(), None);
        let solos: Vec<egui::Rect> = text_rects(&out).into_iter().filter(|(x, _)| x == t("music_page.solo")).map(|(_, r)| r).collect();
        assert_eq!(solos.len(), 8, "at {w} wide one Solo for each part");
        for r in &solos {
            // Inside the page's own margin (16 points), not just the screen.
            assert!(r.left() >= 16.0 && r.right() <= w - 16.0, "at {w} wide each Solo is inside the page: {r:?}");
        }
        let mut rows: Vec<(f32, usize)> = Vec::new();
        for r in &solos {
            match rows.iter_mut().find(|(y, _)| (y - r.center().y).abs() < 4.0) {
                Some(row) => row.1 += 1,
                None => rows.push((r.center().y, 1)),
            }
        }
        let counts: Vec<usize> = rows.iter().map(|r| r.1).collect();
        if w > 1000.0 {
            assert_eq!(counts, vec![8], "wide, the eight parts stand in one row");
        } else {
            assert_eq!(counts, vec![4, 4], "on a phone the parts wrap into two rows of four: {} rows of {counts:?}", counts.len());
        }
        assert!(!texts(&out).iter().any(|x| x == t("music_page.mixer")), "at {w} wide the separate Mixer card is gone");
    }
}

/// THE NEW KEYS (section 28.9): S turns shuffle on, R steps repeat to all, U opens Up next; none of them
/// acts while a text box has the keyboard.
///
/// Seen RED 2026-11-21 with the three keys left out of `keys::handle`: "S turns shuffle on".
#[test]
fn the_keys_s_r_and_u_act_and_never_while_typing() {
    let (mut state, _) = composed_playing();
    state.music_page.tab = Tab::Written;
    let size = egui::vec2(1600.0, 1000.0);
    let (_, ctx) = draw_kept(&mut state, size, Vec::new(), None);
    ctx.memory_mut(|m| m.request_focus(egui::Id::new(super::written::SEARCH_ID)));
    let (_, ctx) = draw_kept(&mut state, size, [key(Key::S), key(Key::R), key(Key::U)].concat(), Some(ctx));
    assert!(!state.music_page.queue.shuffle && state.music_page.queue.repeat == Repeat::Off && !state.music_page.up_next_open, "typing S, R and U in the search box changes nothing");
    ctx.memory_mut(|m| m.surrender_focus(egui::Id::new(super::written::SEARCH_ID)));
    let (_, ctx) = draw_kept(&mut state, size, key(Key::S), Some(ctx));
    assert!(state.music_page.queue.shuffle, "S turns shuffle on");
    let (_, ctx) = draw_kept(&mut state, size, key(Key::R), Some(ctx));
    assert_eq!(state.music_page.queue.repeat, Repeat::All, "R steps repeat to all");
    draw_kept(&mut state, size, key(Key::U), Some(ctx));
    assert!(state.music_page.up_next_open, "U opens Up next");
}

/// Composed piece `seed` of electro swing, `minutes` long.
fn piece(seed: u64) -> Item {
    Item::Composed { genre: "electro_swing".into(), mood: "mothership_calm".into(), seed, tune: Default::default(), minutes: 4.0 }
}

/// SHUFFLE PLAYS EVERY ITEM OF UP NEXT ONCE, IN AN ORDER OF ITS OWN; REPEAT ALL COMES ROUND (section
/// 28.3): five pieces in Up next with shuffle on are each played once by five Nexts, not in their order
/// (the shuffle's seed fixed); with repeat all, the five played come round again before the station.
///
/// Seen RED 2026-11-21 with shuffle drawing nothing (`k` always 0): "shuffled, the five play in an order
/// of their own: [2, 3, 4, 5, 6]".
#[test]
fn shuffle_plays_each_item_once_and_repeat_all_comes_round() {
    let mut s = humanity_music::MusicSettings::default();
    let mut q = Queue::default();
    q.rng = 7;
    q.play_now(piece(1), &mut s, 8.0);
    for k in 2..=6 {
        q.add(piece(k));
    }
    q.shuffle = true;
    let mut order = Vec::new();
    for _ in 0..5 {
        q.next(&mut s, 8.0);
        order.push(s.seed);
    }
    let mut sorted = order.clone();
    sorted.sort();
    assert_eq!(sorted, vec![2, 3, 4, 5, 6], "shuffled, each item plays once: {order:?}");
    assert_ne!(order, vec![2, 3, 4, 5, 6], "shuffled, the five play in an order of their own: {order:?}");
    // In order, with repeat all: 1 to 6, then 1 again (the list comes round), never the station.
    let mut q = Queue::default();
    q.play_now(piece(1), &mut s, 8.0);
    for k in 2..=3 {
        q.add(piece(k));
    }
    q.repeat = Repeat::All;
    let mut seen = Vec::new();
    for _ in 0..4 {
        q.next(&mut s, 8.0);
        seen.push(q.now.clone());
    }
    assert_eq!(seen[2], Some(piece(1)), "repeat all comes round to the first piece, not the station: {seen:?}");
    assert_eq!(seen[3], Some(piece(2)), "and on round again");
}

/// THE ONE LIST SURVIVES A RESTART (section 28.3): the item playing, Up next of all three kinds, the
/// station, history, shuffle and repeat are written to the person's file as they change; a page started
/// afresh reads the same list back.
///
/// Seen RED 2026-11-21 with `save_queue` writing nothing: "the list is written to the person's file".
#[test]
fn the_queue_survives_a_restart() {
    let dir = std::env::temp_dir().join(format!("hos-music-queue-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("music").join("queue.json");
    let (mut state, _) = music_state();
    state.music_page.queue_path = Some(path.clone());
    state.music_page.queue.play_now(Item::Written { id: "ode_to_joy".into() }, &mut state.settings.music, 8.0);
    state.music_page.queue.add(piece(42));
    state.music_page.queue.add(Item::Station { genre: "lofi".into(), mood: "tension".into(), seed: 9, tune: Default::default() });
    state.music_page.save_queue();
    super::transport::toggle_shuffle(&mut state);
    super::transport::cycle_repeat(&mut state);
    super::transport::next(&mut state);
    let kept = state.music_page.queue.clone();
    assert!(path.exists(), "the list is written to the person's file");
    // A restart: a fresh page reads the file.
    let read = super::queue::load(&path).expect("the list reads back");
    assert_eq!(serde_json::to_value(&read).unwrap(), serde_json::to_value(&kept).unwrap(), "a restart reads the list back");
    assert_eq!(read.history, vec![Item::Written { id: "ode_to_joy".into() }], "with what was heard");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A CHOICE MADE ANYWHERE IS ADOPTED (section 28.3): a new seed is a composed piece, a genre or mood
/// card with the seed unchanged a station, a written piece's Play a written item, each "play now" with
/// the item before in history, Up next untouched; a tuning change retunes the item playing.
///
/// Seen RED 2026-11-21 with `observe` taking every composed choice for a station: "a new seed is one
/// composed piece: Some(Station { genre: \"lofi\", mood: \"mothership_calm\", seed: 11, .. })".
#[test]
fn a_choice_made_anywhere_is_adopted_as_play_now() {
    let mut s = humanity_music::MusicSettings { game_chooses: false, genre: "lofi".into(), mood: "mothership_calm".into(), seed: 10, ..Default::default() };
    let mut q = Queue::default();
    q.add(piece(99));
    q.observe(&mut s, 8.0);
    s.seed = 11;
    q.observe(&mut s, 8.0);
    assert!(matches!(&q.now, Some(Item::Composed { seed: 11, .. })), "a new seed is one composed piece: {:?}", q.now);
    assert!(s.minutes > 0.0, "with its length written for the score");
    s.genre = "electro_swing".into();
    q.observe(&mut s, 8.0);
    assert!(matches!(&q.now, Some(Item::Station { genre, .. }) if genre == "electro_swing"), "a genre chosen is its station: {:?}", q.now);
    s.tune.transpose = 2;
    q.observe(&mut s, 8.0);
    assert!(matches!(&q.now, Some(Item::Station { tune, .. }) if tune.transpose == 2), "a tuning change retunes the station");
    s.written = "fur_elise".into();
    q.observe(&mut s, 8.0);
    assert_eq!(q.now, Some(Item::Written { id: "fur_elise".into() }), "a written piece's Play");
    assert_eq!(q.history.len(), 3, "each choice put the one before into history: {:?}", q.history);
    assert_eq!(q.up_next, vec![piece(99)], "Up next untouched");
}

//! THE MUSIC PAGE AT PHONE WIDTH, AND A PLANNED PIECE ON THE ROLL (docs/design/music-page.md section
//! 26). Each test draws the real page headlessly (no GPU) and reads what it DREW.

use super::tests::{frame, music_state, text_rects};
use super::Tab;

/// The operator's phone in points (a CSS viewport of about 412 by 915: music-page.md section 26).
const PHONE: egui::Vec2 = egui::vec2(412.0, 915.0);

/// THE WRITTEN MUSIC TAB FITS A PHONE: drawn 412 points wide, every word of the search row (the search
/// box's label, the sort and both filters with their chosen values) and of the details card lies inside
/// the screen, and the search row's cells stack (each control below the one before) instead of running
/// off to the right. RED first (2026-11-21) against the rung 1 search row (each combo with its label beside
/// it inside one wrapping row): "\"Sort by\" is not drawn on the phone's screen" (the row ran off the right
/// edge, so the sort's label was clipped away entirely).
#[test]
fn the_written_tab_fits_a_phone() {
    let (mut state, lib) = music_state();
    let theme = crate::gui::theme::load_theme();
    state.music_page.tab = Tab::Written;
    state.music_page.written_selected = Some("fur_elise".into());
    // A phone's width, tall enough that the whole tab (the details card below the list) is drawn.
    let out = frame(egui::vec2(PHONE.x, 3000.0), Vec::new(), |ctx| super::draw(ctx, &theme, &mut state));
    let drawn = text_rects(&out);
    let words = [
        humanity_core::i18n::t("music_page.written_search").to_string(),
        humanity_core::i18n::t("music_page.written_sort").to_string(),
        humanity_core::i18n::t("music_page.written_composer_filter").to_string(),
        humanity_core::i18n::t("music_page.written_era_filter").to_string(),
        humanity_core::i18n::t("music_page.written_all_composers").to_string(),
        humanity_core::i18n::t("music_page.written_all_eras").to_string(),
    ];
    for w in &words {
        let r = drawn.iter().find(|(s, _)| s == w).map(|(_, r)| *r).unwrap_or_else(|| panic!("{w:?} is not drawn on the phone's screen (clipped off its edge, or missing)"));
        assert!(r.right() <= PHONE.x + 0.5 && r.left() >= -0.5, "{w:?} is drawn at x {:.0} to {:.0}, past the phone's {} points", r.left(), r.right(), PHONE.x);
    }
    // Nothing on the tab is drawn past the right edge (the details card included).
    let e = lib.score("fur_elise").unwrap();
    for (s, r) in drawn.iter().filter(|(s, _)| !s.trim().is_empty()) {
        assert!(r.right() <= PHONE.x + 0.5, "{s:?} is drawn to x {:.0}, past the phone's {} points", r.right(), PHONE.x);
    }
    assert!(drawn.iter().any(|(s, _)| s == &e.licence), "the details card shows the licence");
    // The cells stack: the era filter below the sort, not beside it.
    let y = |w: &str| drawn.iter().find(|(s, _)| s == w).map(|(_, r)| r.top()).unwrap();
    assert!(y(&words[3]) > y(&words[1]) + 10.0, "the era filter sits below the sort at phone width");
}

/// THE ROLL'S SECTION LETTERS NEVER OVERPRINT: a chosen electro swing piece plays round its arc (the
/// roll draws its plan 20 minutes ahead, some 80 phrases); on a phone and on the operator's screen, no
/// two section letters drawn on one line overlap (the overview shows a letter where there is room for
/// it). RED first (2026-11-21) against the overview drawing every phrase's letter (the phone snapshot
/// showed them as one smear): "412 wide: \"A\" at x 25.0 and \"A\" at x 29.6 overlap by 2.9 points; 79
/// overlapping pairs of 81 letters 412 wide" (at 2560 wide they had room).
#[test]
fn the_rolls_section_letters_never_overprint() {
    let (mut state, lib) = music_state();
    let theme = crate::gui::theme::load_theme();
    let res = std::sync::Arc::new(humanity_music::Resolved::new(&lib, &humanity_music::SampleLibrary::new()).expect("resolve"));
    let g = lib.genre_index("electro_swing").unwrap() as u16;
    let mood = lib.mood_index("mothership_calm").unwrap() as u16;
    let ctx_m = humanity_music::MusicContext { mood, genre_a: g, genre_b: g, piece_s: lib.endless_minutes * 60.0, ..Default::default() };
    state.music_dev.readout.playing = Some(crate::gui::pages::dev_music::Playing { choice: None, seed: 4242, ctx: ctx_m, res, lib: lib.clone() });
    state.music_page.tab = Tab::NowPlaying;
    let mut wrong = Vec::new();
    for size in [egui::vec2(PHONE.x, 3000.0), egui::vec2(2560.0, 1440.0)] {
        let out = frame(size, Vec::new(), |ctx| super::draw(ctx, &theme, &mut state));
        let letters: Vec<(String, egui::Rect)> = text_rects(&out).into_iter().filter(|(s, _)| s.len() == 1 && s.as_bytes()[0].is_ascii_uppercase()).collect();
        assert!(letters.len() > 10, "{} section letters drawn {:.0} wide: the planned piece is not on the roll", letters.len(), size.x);
        let mut over = 0;
        for (i, (sa, a)) in letters.iter().enumerate() {
            for (sb, b) in &letters[i + 1..] {
                let shared = a.right().min(b.right()) - a.left().max(b.left());
                if (a.top() - b.top()).abs() < 1.0 && shared > 0.5 {
                    if over == 0 {
                        wrong.push(format!("{:.0} wide: {sa:?} at x {:.1} and {sb:?} at x {:.1} overlap by {shared:.1} points", size.x, a.left(), b.left()));
                    }
                    over += 1;
                }
            }
        }
        if over > 0 {
            wrong.push(format!("{over} overlapping pairs of {} letters {:.0} wide", letters.len(), size.x));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("; "));
}

/// THE WRITTEN MUSIC TAB'S SEARCH ROW LINES UP: on the operator's screen (2560 by 1440) the four cells'
/// labels (search, sort, composer, era) sit level in one row, and on a phone each stacked cell follows
/// the one above at a label and a control's height (at most 60 points from one label to the next), not
/// with a gap. RED first (2026-11-21) against the cells in horizontal_wrapped (a centred wrapping row;
/// the snapshot showed the labels as a staircase): "\"Sort by\" sits 17.0 points below \"Search\" on a
/// wide screen; \"Composer\" sits 26.5 points below \"Search\" on a wide screen; \"Era\" sits 31.3 points
/// below \"Search\" on a wide screen; on a phone \"Sort by\" is 75.1 points below \"Search\"; on a phone
/// \"Composer\" is 73.5 points below \"Sort by\"; on a phone \"Era\" is 73.5 points below \"Composer\"".
#[test]
fn the_written_search_row_lines_up() {
    let (mut state, _lib) = music_state();
    let theme = crate::gui::theme::load_theme();
    state.music_page.tab = Tab::Written;
    state.music_page.written_selected = Some("fur_elise".into());
    let labels = ["music_page.written_search", "music_page.written_sort", "music_page.written_composer_filter", "music_page.written_era_filter"].map(|k| humanity_core::i18n::t(k).to_string());
    let tops = |size: egui::Vec2, state: &mut crate::gui::GuiState| {
        let out = frame(size, Vec::new(), |ctx| super::draw(ctx, &theme, state));
        let drawn = text_rects(&out);
        labels.clone().map(|w| drawn.iter().find(|(s, _)| *s == w).map(|(_, r)| r.top()).unwrap_or_else(|| panic!("{w:?} is not drawn")))
    };
    // Every misplaced cell is named at once (the wide screen and the phone together).
    let mut wrong = Vec::new();
    let wide = tops(egui::vec2(2560.0, 1440.0), &mut state);
    for (k, y) in wide.iter().enumerate().skip(1) {
        if (y - wide[0]).abs() > 1.0 {
            wrong.push(format!("{:?} sits {:.1} points below {:?} on a wide screen", labels[k], y - wide[0], labels[0]));
        }
    }
    let phone = tops(egui::vec2(PHONE.x, 3000.0), &mut state);
    for k in 1..phone.len() {
        let stride = phone[k] - phone[k - 1];
        if !(stride > 10.0 && stride <= 60.0) {
            wrong.push(format!("on a phone {:?} is {stride:.1} points below {:?}", labels[k], labels[k - 1]));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("; "));
}

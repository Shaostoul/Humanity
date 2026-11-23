//! The first-run restore on a phone (2026-11-22, gui/visible_area.rs). The operator, typing his
//! recovery phrase on his OnePlus 9 Pro (about 412 x 915 points): "when I'm entering my 24 word
//! recovery phrase to join on my phone I can't scroll low enough when the keyboard opens to see
//! the text box preventing me from spelling things correctly."
//!
//! Each scene is the app's own: the onboarding card drawn by `draw_onboarding`, the covered edges
//! taken off the screen the way the frame loop does (`visible_area::apply`, the call
//! `visible_area::before_frame` ends in), the box tapped with the keyboard down and the keyboard
//! then coming up over it. Each test also runs its scene with the covered edges IGNORED and
//! asserts the box then lands under the keyboard, so the scene is one where the fix matters (a
//! gate that cannot fail proves nothing).

use super::{draw_onboarding, PHRASE_BOX_ID};
use crate::gui::theme::Theme;
use crate::gui::visible_area::{self, Covered};
use crate::gui::GuiState;

/// The operator's phone, in points.
const PHONE: egui::Vec2 = egui::vec2(412.0, 915.0);
/// The status bar at the top, and at the bottom the keyboard (about 330 points) sitting on the
/// navigation bar's buttons (48), as on his phone with its three buttons.
const KEYBOARD_UP: Covered = Covered { left: 0.0, top: 24.0, right: 0.0, bottom: 330.0 + 48.0 };
/// The status bar and the navigation bar's buttons, the keyboard down.
const KEYBOARD_DOWN: Covered = Covered { left: 0.0, top: 24.0, right: 0.0, bottom: 48.0 };

/// Twelve words typed, the seventh misspelled ("abandn"): the person checking what they typed.
const HALF_TYPED: &str = "legal winner thank year wave sausage abandn ocean country dumb snake trial";

fn headless() -> (egui::Context, Theme) {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    (ctx, theme)
}

/// The identity step with the restore section open and `typed` in the box.
fn restoring(typed: &str) -> GuiState {
    let mut s = GuiState::default();
    s.onboarding_step = 2;
    s.settings.seed_phrase_show_recover = true;
    s.settings.seed_phrase_input = typed.to_string();
    s
}

/// One headless frame (no GPU) of the onboarding card on a `screen` of that size, `covered` taken
/// off the screen as the frame loop takes it, or not at all when `ignore` (the scene without the fix).
fn frame_on(screen: egui::Vec2, ctx: &egui::Context, theme: &Theme, state: &mut GuiState, covered: Covered, ignore: bool) -> egui::FullOutput {
    let mut input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen)),
        ..Default::default()
    };
    visible_area::apply(ctx, &mut input, if ignore { Covered::default() } else { covered });
    ctx.run(input, |ctx| draw_onboarding(ctx, theme, state))
}

/// The person taps the phrase box with the keyboard down, then the keyboard comes up; the box's
/// rectangle once everything settled (a scroll animates over several frames), whether it still has
/// focus, and the last frame's output.
fn tap_then_keyboard(typed: &str, ignore: bool) -> (egui::Rect, bool, egui::FullOutput) {
    scene(typed, ignore, true)
}

/// [`tap_then_keyboard`], or with `focus` false the keyboard up with the box never tapped (nothing
/// asks for it to be brought into view: the control for the scroll).
fn scene(typed: &str, ignore: bool, focus: bool) -> (egui::Rect, bool, egui::FullOutput) {
    scene_on(PHONE, KEYBOARD_DOWN, KEYBOARD_UP, typed, ignore, focus)
}

/// [`scene`] on a `screen` of that size, with what covers it before (`down`) and after (`up`) the
/// keyboard comes up.
fn scene_on(screen: egui::Vec2, down: Covered, up: Covered, typed: &str, ignore: bool, focus: bool) -> (egui::Rect, bool, egui::FullOutput) {
    let (ctx, theme) = headless();
    let mut state = restoring(typed);
    for _ in 0..3 {
        frame_on(screen, &ctx, &theme, &mut state, down, ignore);
    }
    if focus {
        ctx.memory_mut(|m| m.request_focus(egui::Id::new(PHRASE_BOX_ID)));
    }
    for _ in 0..2 {
        frame_on(screen, &ctx, &theme, &mut state, down, ignore);
    }
    let mut out = None;
    for _ in 0..40 {
        out = Some(frame_on(screen, &ctx, &theme, &mut state, up, ignore));
    }
    let resp = ctx.read_response(egui::Id::new(PHRASE_BOX_ID)).expect("the phrase box was drawn");
    (resp.rect, resp.has_focus(), out.expect("frames ran"))
}

/// The part of the phone a person sees with the keyboard up.
fn above_keyboard() -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(KEYBOARD_UP.left, KEYBOARD_UP.top),
        egui::pos2(PHONE.x - KEYBOARD_UP.right, PHONE.y - KEYBOARD_UP.bottom),
    )
}

/// With the keyboard up, the focused recovery phrase box lies in the part of the phone above the
/// keyboard, with room for a line under it, and inside the phone's width.
///
/// Seen red 2026-11-22 with `keep_in_view` left out of the phrase box (the card fitted to the area
/// above the keyboard, but nothing brought the box up): "the phrase box at [[11.0 590.5] - [399.0
/// 628.6]] is not inside the part above the keyboard [[0.0 24.0] - [412.0 585.0]]". The scene run
/// with the covered edges ignored, below, is the app as it was, and puts the box under the keyboard.
#[test]
fn the_focused_phrase_box_lies_above_the_keyboard() {
    let visible = above_keyboard();
    let (rect, focused, _) = tap_then_keyboard("", false);
    assert!(focused, "the phrase box lost its focus when the keyboard came up");
    assert!(
        rect.top() >= visible.top() - 0.5 && rect.bottom() <= visible.bottom() + 0.5,
        "the phrase box at {rect:?} is not inside the part above the keyboard {visible:?}"
    );
    assert!(rect.left() >= -0.5 && rect.right() <= PHONE.x + 0.5, "the phrase box at {rect:?} runs past the phone's sides");
    // The control: the same scene with the covered edges ignored puts it under the keyboard.
    let (ignored, _, _) = tap_then_keyboard("", true);
    assert!(
        ignored.bottom() > visible.bottom(),
        "the scene does not need the fix: with the keyboard ignored the box sits at {ignored:?}, above {}",
        visible.bottom()
    );
}

/// The phone turned sideways (915 x 412, the navigation bar at the side), half the phrase typed:
/// the part above the keyboard is shorter than what the card holds above the box, so the card's
/// steps scroll and the box is scrolled into view when the keyboard comes up over it.
///
/// Seen red 2026-11-22: the control below, the same keyboard with the box never tapped (so nothing
/// asks for it to be brought into view), leaves it under the keyboard; an earlier run with
/// `keep_in_view` left out of the phrase box failed upright: "the half-typed phrase box at [[11.0
/// 590.5] - [399.0 628.6]] is not inside the part above the keyboard [[0.0 24.0] - [412.0 585.0]]".
#[test]
fn turned_sideways_the_phrase_box_is_scrolled_above_the_keyboard() {
    let screen = egui::vec2(915.0, 412.0);
    let down = Covered { left: 0.0, top: 24.0, right: 48.0, bottom: 0.0 };
    let up = Covered { left: 0.0, top: 24.0, right: 48.0, bottom: 200.0 };
    let visible = egui::Rect::from_min_max(egui::pos2(0.0, up.top), egui::pos2(screen.x - up.right, screen.y - up.bottom));
    let (rect, focused, _) = scene_on(screen, down, up, HALF_TYPED, false, true);
    assert!(focused, "the phrase box lost its focus when the keyboard came up");
    assert!(
        rect.bottom() <= visible.bottom() + 0.5 && rect.top() >= visible.top() - 0.5,
        "the half-typed phrase box at {rect:?} is not inside the part above the keyboard {visible:?}"
    );
    let (untapped, _, _) = scene_on(screen, down, up, HALF_TYPED, false, false);
    assert!(untapped.bottom() > visible.bottom(), "the scene does not need the scroll: untapped, the box sits at {untapped:?}");
}

/// The typed words are shown under the box, numbered 1 to 24 as on the paper, with the count, and
/// a word that is not a recovery-phrase word is named by its number.
///
/// Drawn on a screen tall enough for the whole card, so every row is painted (a row scrolled out of
/// the card's view is not drawn at all).
///
/// Seen red 2026-11-22 before `phrase_typed::numbered` was called under the box: "the numbered
/// word \" 1. legal\" is not drawn".
#[test]
fn the_typed_words_are_numbered_under_the_box() {
    let (ctx, theme) = headless();
    let mut state = restoring(HALF_TYPED);
    let mut out = None;
    for _ in 0..10 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(PHONE.x, 1600.0))),
            ..Default::default()
        };
        out = Some(ctx.run(input, |ctx| draw_onboarding(ctx, &theme, &mut state)));
    }
    let out = out.expect("frames ran");
    let box_rect = ctx.read_response(egui::Id::new(PHRASE_BOX_ID)).expect("the phrase box was drawn").rect;
    let shapes = &out.shapes;
    let find = |text: &str| crate::gui::screen_surface::find_text_in_shapes(shapes, text).filter(|f| f.text == text);
    for (n, word) in HALF_TYPED.split_whitespace().enumerate() {
        let label = format!("{:>2}. {word}", n + 1);
        assert!(find(&label).is_some(), "the numbered word {label:?} is not drawn");
    }
    let count = humanity_core::i18n::tn("phrase_typed.count", 12, &[("total", &24)]);
    assert!(find(&count).is_some(), "the count {count:?} is not drawn");
    let unknown = humanity_core::i18n::tf("phrase_typed.unknown", &[("list", &"7")]);
    assert!(find(&unknown).is_some(), "the misspelled seventh word is not named: {unknown:?}");
    // The numbered words are under the box, not beside or above it.
    let first = find(" 1. legal").expect("the first word");
    assert!(first.rect.top() >= box_rect.bottom() - 0.5, "the numbered words start at {:?}, not under the box {box_rect:?}", first.rect);
}

/// Every word recognised as the phrase's own, and a misspelled one not, whatever its case.
#[test]
fn typed_words_are_checked_against_the_word_list() {
    let words = crate::gui::widgets::phrase_typed::words("Legal  winner\nabandn ");
    assert_eq!(
        words,
        vec![("legal".to_string(), true), ("winner".to_string(), true), ("abandn".to_string(), false)]
    );
    assert!(crate::gui::widgets::phrase_typed::words("   ").is_empty());
}

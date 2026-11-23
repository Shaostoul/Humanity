//! Tests of the visible area (visible_area.rs, 2026-11-22). Each was seen to fail before it passed;
//! what it saw red is in its comment. The pages' own scenes (the first-run restore) are in
//! pages/main_menu_restore_tests.rs.

use super::*;

/// The operator's phone, in points.
const PHONE: egui::Vec2 = egui::vec2(412.0, 915.0);

/// Android's two reports combine to the area inside both: the insets (the bars, the keyboard) and
/// the content rectangle, whichever covers more at each edge. A content rectangle not yet set
/// (all zero) and missing insets cover nothing, and a report that would leave too little of the
/// window is not believed.
///
/// Seen red 2026-11-22 with the content rectangle alone (`insets` unused), the edge-to-edge case,
/// where only the insets know the bars and the keyboard: "left: CoveredPx { left: 0, top: 0, right:
/// 0, bottom: 0 } right: CoveredPx { left: 0, top: 80, right: 0, bottom: 900 }".
#[test]
fn covered_edges_combine_the_insets_and_the_content_rect() {
    let window = (1080, 2400);
    // Edge to edge: the content rectangle is the whole window; the insets hold the status bar,
    // the navigation bar and the keyboard.
    assert_eq!(
        covered_px(window, Some([0, 80, 0, 900]), Some([0, 0, 1080, 2400])),
        CoveredPx { left: 0, top: 80, right: 0, bottom: 900 }
    );
    // Fitted (before Android 15): the content rectangle shrinks for the bars and the keyboard too.
    assert_eq!(
        covered_px(window, Some([0, 80, 0, 126]), Some([0, 80, 1080, 1500])),
        CoveredPx { left: 0, top: 80, right: 0, bottom: 900 }
    );
    // Turned, the navigation bar at the side.
    assert_eq!(covered_px((2400, 1080), Some([0, 80, 126, 0]), None), CoveredPx { left: 0, top: 80, right: 126, bottom: 0 });
    // Nothing reported yet: nothing covered.
    assert_eq!(covered_px(window, None, Some([0, 0, 0, 0])), CoveredPx::default());
    assert_eq!(covered_px(window, None, None), CoveredPx::default());
    // A report leaving under 15% of the height is not believed.
    assert_eq!(covered_px(window, Some([0, 80, 0, 2100]), None), CoveredPx::default());
    // Negative insets cover nothing.
    assert_eq!(covered_px(window, Some([-5, -5, -5, -5]), None), CoveredPx::default());
}

/// Pixels become the same share of egui's screen in points, and `apply` takes them off the
/// screen rectangle and remembers them; zero edges change nothing.
///
/// Seen red 2026-11-22 with `apply` remembering the edges but leaving the screen rectangle whole:
/// "the screen rectangle is still Some([[0 0] - [412 915]])".
#[test]
fn applied_edges_shrink_the_screen_and_are_remembered() {
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, PHONE);
    // A 1080 x 2400 pixel window shown as 412 x 915 points: 900 pixels are 343 points.
    let pts = in_points(CoveredPx { left: 0, top: 63, right: 0, bottom: 900 }, (1080, 2400), screen);
    assert!((pts.bottom - 900.0 / 2400.0 * 915.0).abs() < 0.01 && (pts.top - 63.0 / 2400.0 * 915.0).abs() < 0.01, "{pts:?}");
    let ctx = egui::Context::default();
    let mut input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
    apply(&ctx, &mut input, Covered { left: 0.0, top: 24.0, right: 0.0, bottom: 330.0 });
    assert_eq!(
        input.screen_rect,
        Some(egui::Rect::from_min_max(egui::pos2(0.0, 24.0), egui::pos2(412.0, 585.0))),
        "the screen rectangle is still {:?}",
        Some(screen)
    );
    assert_eq!(covered(&ctx).bottom, 330.0);
    let mut whole = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
    apply(&ctx, &mut whole, Covered::default());
    assert_eq!(whole.screen_rect, Some(screen));
    assert!(covered(&ctx).is_zero());
}

/// The app's frame loop takes the covered edges off the input after the Font Size sets the scale
/// and before the pass lays anything out, so every page gets only the visible area.
///
/// Seen red 2026-11-22 before engine/frame_ui_run.rs called it: "the frame loop does not call
/// visible_area::before_frame".
#[test]
fn the_frame_loop_gives_egui_only_the_visible_area() {
    let src = std::fs::read_to_string(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("src/engine/frame_ui_run.rs")).expect("frame_ui_run.rs");
    let take = src.find("take_egui_input(&gpu.window)").expect("the main window's input");
    let size = src.find("font_size::before_frame(").expect("the Font Size");
    let here = src.find("visible_area::before_frame(").expect("the frame loop does not call visible_area::before_frame");
    let run = src.find("state.egui_ctx.run(raw_input").expect("the pass");
    assert!(take < size && size < here && here < run, "visible_area::before_frame must come after the Font Size and before the pass");
    assert!(src.contains("visible_area::platform_covered(&gpu.window)"), "the frame loop measures nothing");
    // And the drawing-on-change loop draws a frame when what covers the window changes.
    let redraw = std::fs::read_to_string(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("src/engine/redraw.rs")).expect("redraw.rs");
    assert!(redraw.contains("visible_area::platform_changed("), "a keyboard coming up while the loop sleeps draws nothing");
}

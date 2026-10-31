//! From gui/pages/maps/tests.rs: the Maps page names the ship and its real height, fed as the
//! engine's frame feeds it (`map_feed::frame_update`).

use crate::gui::pages::maps::tests::drawn_texts;

/// The page as opening it from the game leaves it (Maps increment 1) names
/// the ship by its one name and says its real height, in the Stations row
/// (computed from data/stations/home.ron) and on the map, and "400 km LEO"
/// appears nowhere.
///
/// Seen red 2026-10-21 with the body list's old hard-coded row put back:
/// "the page still says 400 km LEO: [.. "Home Station · 400 km LEO" ..]".
#[test]
fn the_page_names_the_ship_and_its_real_height_never_400_km_leo() {
    crate::gui::pages::maps::set_clock_for_snapshot(Some(843_782_400.0));
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    let mut state = crate::gui::GuiState::default();
    let me = crate::gui::pages::maps::camera::tests::feed_at(3600.0, glam::DVec3::ZERO);
    crate::engine::map_feed::frame_update(&mut state.maps, me, true, false);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 900.0))),
        ..Default::default()
    };
    let out = ctx.run(input, |ctx| crate::gui::pages::maps::draw(ctx, &theme, &mut state));
    crate::gui::pages::maps::set_clock_for_snapshot(None);
    let all = drawn_texts(&out);
    assert!(!all.iter().any(|t| t.contains("400 km") || t.contains("LEO")), "the page still says 400 km LEO: {all:?}");
    for want in ["UHS Mothership · your home", "35,870 km up · synchronous, hangs over one place", "UHS Mothership", "your home, you are aboard · 35,870 km up", "Following you"] {
        assert!(all.iter().any(|t| t == want), "the page does not say {want:?}: {all:?}");
    }
}

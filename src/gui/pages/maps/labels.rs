//! Names on the Solar System view (Maps increment 2, docs/design/maps-zoom-to-anything.md
//! 5.5): every body's name goes through the map label placer
//! (`widgets/label_placer.rs`, already used by the Galaxy and Planet views),
//! offered in PRIORITY order, "you, the anchor, the Sun, planets by apparent
//! size, dwarf planets, moons only when their parent's disc or orbit is large
//! on screen". The first clear spot (right, left, above, below) wins; a name
//! with none is dropped, so the more important name keeps its place.
//!
//! Until this increment the names were pills drawn over each other by a
//! collision dodge that never dropped one; moons and asteroids had no names
//! unless hovered. A pill is kept only for the body whose card is open.
//!
//! Each name has a dark edge (`map_label_halo`) so it reads over a bright
//! globe in the world layer as well as over black space.

use egui::{Pos2, Rect, Vec2};

use crate::gui::theme::Theme;
use crate::gui::widgets::label_placer::{point_candidates, LabelPlacer};

/// One name offered to the placer.
#[derive(Debug, Clone)]
pub struct LabelOffer {
    pub id: String,
    pub text: String,
    /// The body's centre on the canvas and the radius it is drawn at.
    pub at: Pos2,
    pub r: f32,
    /// Lower is offered first.
    pub priority: f32,
    /// Drawn smaller and dimmer (moons, minor bodies).
    pub minor: bool,
}

/// The priority of a body's name: lower first. `forced` (hovered, selected,
/// anchored) comes right after you; then the Sun; planets by how big they
/// look; dwarf planets; moons; the rest.
pub fn priority(id: &str, body_type: &str, r_px: f32, forced: bool) -> f32 {
    if forced {
        return 1.0;
    }
    let size = 1.0 / (1.0 + r_px.max(0.0)); // bigger on screen, earlier
    match (id, body_type) {
        ("sun", _) => 2.0,
        (_, "terrestrial" | "gas_giant" | "ice_giant") => 3.0 + size,
        (_, "dwarf_planet") => 5.0 + size,
        (_, "moon") => 7.0 + size,
        _ => 9.0 + size,
    }
}

/// Whether a body's name is offered at all: a moon only when it stands
/// clear of its planet on screen (`moon_spread_px` from it), a minor body
/// (asteroid, comet) only when the camera is close (`near`), anything when
/// forced.
pub fn offered(body_type: &str, moon_spread_px: f32, near: bool, forced: bool) -> bool {
    forced
        || match body_type {
            "moon" => moon_spread_px > 22.0,
            "asteroid" | "comet" | "artificial" => near,
            _ => true,
        }
}

/// Place and paint every offer in priority order. Returns the names placed,
/// with their rectangles (for clicks), and how many were dropped.
pub fn place_and_paint(paint: &egui::Painter, theme: &Theme, placer: &mut LabelPlacer, mut offers: Vec<LabelOffer>) -> (Vec<(String, Rect)>, u32) {
    offers.sort_by(|a, b| a.priority.partial_cmp(&b.priority).unwrap_or(std::cmp::Ordering::Equal));
    let mut placed = Vec::new();
    let mut dropped = 0;
    for o in offers {
        let (size, colour) = if o.minor { (theme.map_label_size * 0.85, theme.map_label_minor()) } else { (theme.map_label_size, theme.map_label()) };
        let galley = paint.layout_no_wrap(o.text.clone(), egui::FontId::proportional(size), colour);
        match placer.place(point_candidates(o.at, o.r, galley.size(), 3.0)) {
            Some(spot) => {
                paint_with_halo(paint, theme, spot.min, o.text, size, colour);
                placed.push((o.id, spot));
            }
            None => dropped += 1,
        }
    }
    (placed, dropped)
}

/// Paint text at `at` (its top-left) with a one-pixel dark edge all round.
pub fn paint_with_halo(paint: &egui::Painter, theme: &Theme, at: Pos2, text: String, size: f32, colour: egui::Color32) {
    let halo = paint.layout_no_wrap(text.clone(), egui::FontId::proportional(size), theme.map_label_halo());
    for d in [Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, -1.0), Vec2::new(0.0, 1.0)] {
        paint.galley(at + d, halo.clone(), theme.map_label_halo());
    }
    let main = paint.layout_no_wrap(text, egui::FontId::proportional(size), colour);
    paint.galley(at, main, colour);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The order the design asks for: forced first, then the Sun, planets
    /// bigger on screen first, dwarf planets, moons, the rest; and a moon is
    /// named only once it stands clear of its planet.
    ///
    /// Seen red 2026-10-21 with planets ranked by name, not size: "a 2 px
    /// Neptune is offered before a 9 px Jupiter".
    #[test]
    fn names_are_offered_in_the_designs_order() {
        assert!(priority("io", "moon", 1.0, true) < priority("sun", "star", 8.0, false));
        assert!(priority("sun", "star", 8.0, false) < priority("neptune", "ice_giant", 2.0, false));
        assert!(priority("jupiter", "gas_giant", 9.0, false) < priority("neptune", "ice_giant", 2.0, false), "a 2 px Neptune is offered before a 9 px Jupiter");
        assert!(priority("neptune", "ice_giant", 2.0, false) < priority("ceres", "dwarf_planet", 3.0, false));
        assert!(priority("ceres", "dwarf_planet", 3.0, false) < priority("moon", "moon", 3.0, false));
        assert!(priority("moon", "moon", 3.0, false) < priority("vesta", "asteroid", 3.0, false));
        assert!(!offered("moon", 10.0, true, false) && offered("moon", 40.0, false, false));
        assert!(!offered("asteroid", 0.0, false, false) && offered("asteroid", 0.0, true, false) && offered("asteroid", 0.0, false, true));
    }

    /// Names never overlap: a crowd of bodies on top of each other keeps the
    /// most important names and drops the rest, and every placed name is
    /// clear of every other.
    ///
    /// Seen red 2026-10-21 with every name painted at its first choice (no
    /// placer): "sun and mercury overlap".
    #[test]
    fn names_never_overlap() {
        let ctx = egui::Context::default();
        let theme = crate::gui::theme::load_theme();
        let mut placed = Vec::new();
        let mut dropped = 0;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let paint = ctx.layer_painter(egui::LayerId::background());
            let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0));
            let mut placer = LabelPlacer::new(bounds, 3.0);
            let names = ["Sun", "Mercury", "Venus", "Earth", "Mars", "Ceres", "Vesta", "Moon"];
            let offers = names
                .iter()
                .enumerate()
                .map(|(i, n)| LabelOffer { id: n.to_lowercase(), text: n.to_string(), at: Pos2::new(200.0 + i as f32 * 3.0, 150.0), r: 3.0, priority: i as f32, minor: false })
                .collect();
            let (p, d) = place_and_paint(&paint, &theme, &mut placer, offers);
            placed = p;
            dropped = d;
        });
        assert_eq!(placed[0].0, "sun", "the most important name lost its place");
        for (i, (a, ra)) in placed.iter().enumerate() {
            for (b, rb) in &placed[i + 1..] {
                assert!(!ra.intersects(*rb), "{a} and {b} overlap");
            }
        }
        // Eight points within 21 px: the first two take the right and the
        // left, and their names block the spots above and below the rest.
        assert!(placed.len() >= 2, "only {} names placed", placed.len());
        assert!(dropped >= 1, "eight names round one point all found room");
    }
}

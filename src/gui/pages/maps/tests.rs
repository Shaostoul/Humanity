//! The Maps page's unit tests (Phase 4d-quad, v0.248.0, and later): Track
//! and Focus, the sub-solar point, the geographic readout, and the shipped
//! star map. Moved out of `gui/pages/cosmos.rs` in Maps increment 0
//! (2026-10-20); the bodies are unchanged.

use crate::cosmos::{body_world_position_3d_au, find_body};

use super::galaxy_view::parse_stars_map;
use super::sky_events::{format_geo, subpoint_lat_lon_deg};
use super::system_view::{focus_should_clear_tracking, toggle_tracked_body};


/// Clicking Track on a body that isn't currently tracked starts
/// tracking it.
#[test]
fn toggle_tracked_body_starts_tracking() {
    assert_eq!(toggle_tracked_body(None, "earth"), Some("earth".to_string()));
    assert_eq!(toggle_tracked_body(Some("mars"), "earth"), Some("earth".to_string()));
}

/// Clicking Track again on the body already being tracked stops
/// tracking (the "Stop Tracking" toggle-off case).
#[test]
fn toggle_tracked_body_stops_tracking_the_same_body() {
    assert_eq!(toggle_tracked_body(Some("earth"), "earth"), None);
}

/// A Focus request for a body OTHER than the tracked one cancels
/// tracking -- manually focusing elsewhere means the user wants to
/// look away from whatever they were following.
#[test]
fn focus_on_different_body_clears_tracking() {
    assert!(focus_should_clear_tracking(Some("earth"), "mars"));
    assert!(focus_should_clear_tracking(None, "mars"));
}

/// A Focus request for the SAME body currently being tracked does NOT
/// cancel tracking -- this is exactly the initial snap-to that
/// starting Track itself issues, and re-focusing on what's already
/// being followed shouldn't interrupt the follow.
#[test]
fn focus_on_tracked_body_itself_preserves_tracking() {
    assert!(!focus_should_clear_tracking(Some("earth"), "earth"));
}

/// The sub-SOLAR point's geographic latitude must oscillate between
/// roughly +23.44° (June solstice, Tropic of Cancer) and −23.44°
/// (December solstice, Tropic of Capricorn) over one year. This
/// pins down BOTH the obliquity magnitude AND the sign of the
/// celestial-pole derivation in `subpoint_lat_lon_deg` — if the
/// ecliptic→equatorial rotation were inverted the extremes would
/// flip sign or vanish. Tolerance is generous (±2°) because the
/// orbital model uses mean (not true-of-date) elements.
#[test]
fn subsolar_latitude_tracks_obliquity() {
    let earth = find_body("earth").expect("earth in embedded sol dataset");
    let sun = find_body("sun").expect("sun in embedded sol dataset");
    let mut min_lat = f64::INFINITY;
    let mut max_lat = f64::NEG_INFINITY;
    // Daily samples across ~13 months from J2000 so a full cycle is
    // guaranteed regardless of where the mean anomaly starts.
    for day in 0..400 {
        let t = day as f64 * 86_400.0;
        let earth_pos = body_world_position_3d_au(earth, t);
        let sun_pos = body_world_position_3d_au(sun, t); // ≈ origin
        let (lat, lon) = subpoint_lat_lon_deg(sun_pos, earth_pos, t);
        assert!(
            lon > -180.0001 && lon <= 180.0001,
            "longitude {lon} out of (-180,180]"
        );
        assert!(
            lat.is_finite() && lat.abs() <= 90.0,
            "latitude {lat} not a valid geographic latitude"
        );
        min_lat = min_lat.min(lat);
        max_lat = max_lat.max(lat);
    }
    assert!(
        (max_lat - 23.44).abs() < 2.0,
        "sub-solar max latitude {max_lat:.2}° should peak near +23.44° \
         (Tropic of Cancer), obliquity sign/magnitude wrong?"
    );
    assert!(
        (min_lat + 23.44).abs() < 2.0,
        "sub-solar min latitude {min_lat:.2}° should bottom near −23.44° \
         (Tropic of Capricorn), obliquity sign/magnitude wrong?"
    );
}

/// At the vernal equinox the Sun crosses the celestial equator, so
/// the sub-solar latitude passes through 0°. Verify the curve
/// actually crosses zero (it must, given the ±23° extremes, but this
/// guards against a degenerate constant-latitude bug).
#[test]
fn subsolar_latitude_crosses_equator() {
    let earth = find_body("earth").unwrap();
    let sun = find_body("sun").unwrap();
    let mut saw_pos = false;
    let mut saw_neg = false;
    for day in 0..400 {
        let t = day as f64 * 86_400.0;
        let (lat, _) = subpoint_lat_lon_deg(
            body_world_position_3d_au(sun, t),
            body_world_position_3d_au(earth, t),
            t,
        );
        if lat > 1.0 {
            saw_pos = true;
        }
        if lat < -1.0 {
            saw_neg = true;
        }
    }
    assert!(
        saw_pos && saw_neg,
        "sub-solar latitude never spanned both hemispheres"
    );
}

#[test]
fn format_geo_quadrants() {
    assert_eq!(format_geo(12.34, 56.78), "12.3°N 56.8°E");
    assert_eq!(format_geo(-12.34, -56.78), "12.3°S 56.8°W");
    assert_eq!(format_geo(0.0, 0.0), "0.0°N 0.0°E");
}

// `shipped_seattle_region_is_real` moved to
// `crate::terrain::osm_region` (v0.1148) along with the parser it
// exercises. It is the same assertions on the same shipped file.

/// Independent check of the SHIPPED stars-map.bin against known
/// astronomy: the parser and the generator (scripts/build-stars-map-bin.
/// js) are locked together through the real file, and the file itself is
/// proven to carry real positions (Sirius at 8.6 ly, brightest-first
/// ordering), not just plausible bytes.
#[test]
fn shipped_stars_map_bin_is_real() {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/data/stars-map.bin"))
        .expect("data/stars-map.bin ships with the repo");
    let stars = parse_stars_map(&bytes).expect("parses");
    assert!(stars.len() > 100_000, "full HYG catalog, got {}", stars.len());
    // Brightest-first ordering is what makes the zoom cutoff a `break`.
    assert!(
        stars.windows(2).all(|w| w[0].mag <= w[1].mag),
        "records must be sorted by magnitude ascending"
    );
    // Sirius: brightest star in the night sky, 8.6 ly out.
    let sirius = stars
        .iter()
        .find(|s| s.name.as_deref() == Some("Sirius"))
        .expect("Sirius is named in the catalog");
    let dist = (sirius.x as f64 * sirius.x as f64
        + sirius.y as f64 * sirius.y as f64
        + sirius.z as f64 * sirius.z as f64)
        .sqrt();
    assert!((dist - 8.6).abs() < 0.3, "Sirius at {dist:.2} ly, expected ~8.6");
    assert!(sirius.mag < -1.0, "Sirius mag {}, expected ~-1.44", sirius.mag);
    // Vega: nearly on the Sun's magnitude-0 doorstep at 25 ly.
    let vega = stars
        .iter()
        .find(|s| s.name.as_deref() == Some("Vega"))
        .expect("Vega is named in the catalog");
    let vdist = (vega.x as f64 * vega.x as f64
        + vega.y as f64 * vega.y as f64
        + vega.z as f64 * vega.z as f64)
        .sqrt();
    assert!((vdist - 25.0).abs() < 1.0, "Vega at {vdist:.2} ly, expected ~25");
}

/// Every text a frame of the page drew, whole.
fn drawn_texts(out: &egui::FullOutput) -> Vec<String> {
    fn visit(shape: &egui::Shape, all: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => all.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| visit(s, all)),
            _ => {}
        }
    }
    let mut all = Vec::new();
    for cs in &out.shapes {
        visit(&cs.shape, &mut all);
    }
    all
}

/// The page as opening it from the game leaves it (Maps increment 1) names
/// the ship by its one name and says its real height, in the Stations row
/// (computed from data/stations/home.ron) and on the map, and "400 km LEO"
/// appears nowhere.
///
/// Seen red 2026-10-21 with the body list's old hard-coded row put back:
/// "the page still says 400 km LEO: [.. "Home Station · 400 km LEO" ..]".
#[test]
fn the_page_names_the_ship_and_its_real_height_never_400_km_leo() {
    super::set_clock_for_snapshot(Some(843_782_400.0));
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    let mut state = crate::gui::GuiState::default();
    let me = super::camera::tests::feed_at(3600.0, glam::DVec3::ZERO);
    crate::engine::map_feed::frame_update(&mut state.maps, me, true, false);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 900.0))),
        ..Default::default()
    };
    let out = ctx.run(input, |ctx| super::draw(ctx, &theme, &mut state));
    super::set_clock_for_snapshot(None);
    let all = drawn_texts(&out);
    assert!(!all.iter().any(|t| t.contains("400 km") || t.contains("LEO")), "the page still says 400 km LEO: {all:?}");
    for want in ["UHS Mothership · your home", "35,870 km up · synchronous, hangs over one place", "UHS Mothership", "your home, you are aboard · 35,870 km up", "Following you"] {
        assert!(all.iter().any(|t| t == want), "the page does not say {want:?}: {all:?}");
    }
}

/// Track on a body's card survives the frame it is pressed: the card asks for
/// the flight (a focus request, consumed next frame), and the end-of-frame
/// tracker must not see "the camera is not on that body" and clear it. Found by
/// the critic review of Maps increment 1 (2026-10-21).
///
/// Seen red 2026-10-21 with the pending focus not considered: "Track reverted
/// the frame it was pressed".
#[test]
fn track_survives_the_frame_it_is_pressed() {
    let mut state = crate::gui::GuiState::default();
    state.maps.tracked_body = Some("moon".into());
    state.maps.focus_request = Some("moon".into());
    super::controls::advance(&mut state);
    assert_eq!(state.maps.tracked_body.as_deref(), Some("moon"), "Track reverted the frame it was pressed");
    state.maps.focus_request = None;
    super::controls::advance(&mut state);
    assert_eq!(state.maps.tracked_body, None, "with no flight under way and the camera elsewhere, tracking ends");
}

/// The page with no feed opens on the overview LOOKING DOWN FROM ECLIPTIC
/// NORTH, and from there the planets go round the Sun counter-clockwise, as
/// they really do seen from the north (BUG-140 made the model's orbits turn
/// that way). Before Maps increment 1 the default camera sat about 56 AU BELOW
/// the ecliptic looking up (pitch -1.2 with `y = distance * sin(pitch)`), where
/// the same orbits look clockwise; found by BUG-140's fixer, 2026-10-21.
///
/// Seen red 2026-10-21 with the overview pitched up instead of down
/// (OVERVIEW_PITCH_RAD +70 degrees): "mercury goes round the Sun clockwise on
/// the overview (a x b = 15.3)"; with the eye check first, "the overview's eye
/// is 56.4 AU BELOW the ecliptic".
#[test]
fn the_overview_looks_down_from_ecliptic_north_and_orbits_turn_counter_clockwise() {
    use super::camera::{MapCamera, MapProjector, MapScene};
    let t0 = 843_782_400.0;
    let scene = MapScene::new(None, t0, false);
    let cam = MapCamera::default();
    let pose = cam.pose(&scene);
    let sun0 = scene.body_m("sun").expect("the Sun");
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, 900.0));
    let proj = MapProjector::new(pose, rect);
    let (sun_px, _) = proj.project(sun0).expect("the Sun is in view");
    for id in ["mercury", "venus", "earth", "mars", "jupiter"] {
        // Each body's place round the Sun at t0 and 10 days on, drawn from the
        // t0 camera (heliocentric, so Earth's own motion does not move the frame).
        let at = |t: f64| {
            let s = MapScene::new(None, t, false);
            let helio = s.body_m(id).unwrap() - s.body_m("sun").unwrap();
            let (p, _) = proj.project(sun0 + helio).expect("in view");
            p - sun_px
        };
        let (a, b) = (at(t0), at(t0 + 10.0 * 86_400.0));
        // Screen y points down, so counter-clockwise on screen is a NEGATIVE
        // a x b in screen coordinates.
        let cross = a.x * b.y - a.y * b.x;
        assert!(cross < 0.0, "{id} goes round the Sun clockwise on the overview (a x b = {cross:.1})");
    }
    let above = (pose.eye - sun0).y / crate::cosmos::M_PER_AU;
    assert!(above > 0.0, "the overview's eye is {:.1} AU BELOW the ecliptic", -above);
}

/// The reference rings (LEO, GEO...) lie in the body's equatorial plane, the
/// world's XZ plane (the game has no axial tilt), at their radius. Until Maps
/// increment 1 they were drawn in the XY plane, standing on edge to the orbits
/// they name; found again by BUG-140's fixer, 2026-10-21.
///
/// Seen red 2026-10-21 with the ring built in the XY plane as before:
/// "a reference ring leaves the equatorial plane by 42164.0 km".
#[test]
fn reference_rings_lie_in_the_equatorial_plane() {
    let centre = glam::DVec3::new(1.0e7, -3.0e6, 2.0e7);
    let r = 42_164_000.0;
    let ring = super::system_view::reference_ring_points(centre, r, 96);
    assert_eq!(ring.len(), 96);
    let off = ring.iter().map(|p| (p.y - centre.y).abs()).fold(0.0, f64::max);
    assert!(off < 1e-6, "a reference ring leaves the equatorial plane by {:.1} km", off / 1000.0);
    for p in &ring {
        assert!(((*p - centre).length() / r - 1.0).abs() < 1e-12, "a ring point is off its radius");
    }
}

/// L4 leads the smaller body round its orbit by 60 degrees and L5 trails it,
/// on every date, and all five points are real numbers. The overlay used
/// "ecliptic up = +Z", which lies IN the ecliptic of the engine's frame (Y up):
/// its orbit normal came out as -sign(cos a) Y, so L4 and L5 swapped every half
/// orbit, and NaN when the body crossed the Z axis. Found by BUG-140's fixer,
/// 2026-10-21; the normal is now the child's own motion round its parent.
///
/// Seen red 2026-10-21 on the old normal: "sun-earth on day 0: L4 trails the
/// child (along its motion -0.874)".
#[test]
fn lagrange_l4_leads_and_l5_trails_on_every_date() {
    use crate::cosmos::{body_world_position_3d_au, find_body};
    for (parent, child) in [("sun", "earth"), ("earth", "moon"), ("sun", "jupiter")] {
        let (p, c) = (find_body(parent).unwrap(), find_body(child).unwrap());
        for day in (0..360).step_by(15) {
            let t = 843_782_400.0 + day as f64 * 86_400.0;
            let pts = super::system_view::compute_lagrange_points(p, c, t).expect("five points");
            assert!(pts.iter().all(|q| q.is_finite()), "{parent}-{child} on day {day}: a point is not a number");
            let pp = body_world_position_3d_au(p, t);
            let sep = body_world_position_3d_au(c, t) - pp;
            let sep2 = body_world_position_3d_au(c, t + 3600.0) - body_world_position_3d_au(p, t + 3600.0);
            let motion = (sep2 - sep).normalize();
            let lead = |q: glam::DVec3| (q - pp).normalize().dot(motion);
            assert!(lead(pts[3]) > 0.5, "{parent}-{child} on day {day}: L4 trails the child (along its motion {:.3})", lead(pts[3]));
            assert!(lead(pts[4]) < -0.5, "{parent}-{child} on day {day}: L5 leads the child (along its motion {:.3})", lead(pts[4]));
        }
    }
}

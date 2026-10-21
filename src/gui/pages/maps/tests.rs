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

//! Child of gui/pages/maps/sky_events.rs (#[path]): the sub-point's LONGITUDE,
//! which the latitude tests in `maps/tests.rs` never looked at (BUG-140,
//! 2026-10-21), checked against JPL Horizons.

use super::*;

/// JPL Horizons, fetched 2026-10-21 from
/// https://ssd.jpl.nasa.gov/api/horizons.api: each body's apparent geocentric
/// right ascension and declination (observer 500@399, QUANTITIES=2, degrees)
/// and the apparent sidereal time at longitude 0 (observer coord@399 at 0,0,0,
/// QUANTITIES=7), at the UT moments given. The point a body stands over is
/// latitude = declination and east longitude = RA - Greenwich sidereal time.
/// Columns: body, UT, the engine's sim seconds, RA, Dec, then the sidereal time
/// as h, m, s exactly as Horizons printed it.
const HORIZONS: &[(&str, &str, f64, f64, f64, (f64, f64, f64))] = &[
    ("sun", "2026-06-21 12:00", 835315200.0, 90.155654652, 23.437851309, (5.0, 58.0, 48.3326)),
    ("sun", "2026-10-21 00:00", 845812800.0, 205.636084512, -10.623341173, (1.0, 57.0, 49.7816)),
    ("sun", "2026-12-21 18:00", 851148000.0, 269.868816157, -23.437361956, (0.0, 1.0, 17.1037)),
    ("mars", "2026-06-21 12:00", 835315200.0, 52.503824362, 18.558568840, (5.0, 58.0, 48.3326)),
    ("mars", "2026-10-21 00:00", 845812800.0, 135.775437231, 18.262851405, (1.0, 57.0, 49.7816)),
    ("mars", "2026-12-21 18:00", 851148000.0, 160.918191091, 11.228480318, (0.0, 1.0, 17.1037)),
];

fn wrap180(d: f64) -> f64 {
    (d + 180.0).rem_euclid(360.0) - 180.0
}

/// The sub-solar and sub-Mars points against Horizons, latitude AND longitude.
/// Measured 2026-10-21 within 0.025 degree (the model's own orbit error plus
/// nutation and aberration, which the readout ignores), so 0.1 is the gate.
/// Red check, run: the mirrored frame turned right ascension backwards, putting
/// the June sub-solar point at 179.5 W instead of 0.5 E, and the J2000 right
/// ascension paired with sidereal time of date left a further 0.37 degree in
/// 2026.
#[test]
fn subpoints_match_jpl_horizons_in_latitude_and_longitude() {
    let earth = find_body("earth").expect("earth");
    let mut table = String::new();
    let mut worst = 0.0_f64;
    for &(id, when, t, ra, dec, (h, m, s)) in HORIZONS {
        let body = find_body(id).expect(id);
        let want_lon = wrap180(ra - (h + m / 60.0 + s / 3600.0) * 15.0);
        let (lat, lon) = subpoint_lat_lon_deg(
            body_world_position_3d_au(body, t),
            body_world_position_3d_au(earth, t),
            t,
        );
        let (dlat, dlon) = (lat - dec, wrap180(lon - want_lon));
        worst = worst.max(dlat.abs()).max(dlon.abs());
        table.push_str(&format!(
            "\n  {id:<5} {when}: Horizons {dec:8.3} {want_lon:9.3} | page {lat:8.3} {lon:9.3} ({})",
            format_geo(lat, lon)
        ));
    }
    println!("the point each body stands over, Horizons vs the Maps page:{table}");
    assert!(worst < 0.1, "a sub-point is off (0.1 degree allowed):{table}");
}

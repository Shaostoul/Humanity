//! Child of gui/pages/maps/sky_events.rs (#[path]): the sub-point's LONGITUDE,
//! which the latitude tests in `maps/tests.rs` never looked at (BUG-140,
//! 2026-10-21), checked against JPL Horizons; and the solar eclipses the page
//! finds, checked against NASA's catalog (the BUG-140 follow-up, same day).

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

/// NASA's solar eclipses of 2026 and 2027 (F. Espenak, "Five Millennium Catalog
/// of Solar Eclipses: 2001 to 2100",
/// https://eclipse.gsfc.nasa.gov/SEcat5/SE2001-2100.html, page dated 2010 Jul
/// 21, fetched 2026-10-21). As the catalog prints them: date, TD of greatest
/// eclipse, Delta T (s), type, gamma:
///   2026 Feb 17  12:13:06  75  A  -0.9743
///   2026 Aug 12  17:47:06  75  T   0.8977
///   2027 Feb 06  16:00:48  76  A  -0.2952
///   2027 Aug 02  10:07:50  76  T   0.1421
/// "Greatest Eclipse is the instant when the axis of the Moon's shadow passes
/// closest to Earth's center"; the UT below is TD less Delta T. NASA lists no
/// other solar eclipse in these two years. Columns: UT, the engine's sim
/// seconds, the kind the page must call it (None: not checked, see the row).
const NASA_SOLAR_ECLIPSES: &[(&str, f64, Option<&str>)] = &[
    // Gamma -0.974: the shadow's axis grazes Earth's edge. The model's Moon is
    // a few hundredths of a degree too far from the Sun there, enough for the
    // axis to just miss, so the page calls it partial. Its day and hour are
    // checked, its kind is not.
    ("2026-02-17 12:11:51", 824_602_311.0, None),
    ("2026-08-12 17:45:51", 839_828_751.0, Some("total solar eclipse")),
    ("2027-02-06 15:59:32", 855_201_572.0, Some("annular solar eclipse")),
    ("2027-08-02 10:06:34", 870_473_194.0, Some("total solar eclipse")),
];

/// How far the model's greatest eclipse may sit from NASA's, hours. The Moon's
/// place is a precessing Kepler ellipse without its periodic perturbations, up
/// to 2.5 degrees off in longitude (`cosmos::tests::the_moon_stands_at_its_jpl_horizons_latitude_and_longitude`),
/// which is 4.6 hours of its motion against the Sun.
const ECLIPSE_HOURS: f64 = 5.0;

/// Every solar eclipse the page's detector sees in 2026 and 2027, by sweeping
/// it: (sim seconds at the least Sun-Moon separation of each run of eclipse
/// samples, which is NASA's greatest eclipse, and the kind it reports there).
fn swept_solar_eclipses(start: f64, end: f64) -> Vec<(f64, &'static str)> {
    let mut found = Vec::new();
    let mut best: Option<(f64, f64, &'static str)> = None; // (sep, t, kind)
    let mut t = start;
    while t <= end {
        match (detect_sky_events(t).1, best) {
            (Some(e), b) if b.map_or(true, |(s, _, _)| e.separation_deg < s) => {
                best = Some((e.separation_deg, t, e.kind.label()));
            }
            (Some(_), _) => {}
            (None, Some((_, bt, _))) => {
                // Refine the least separation to 5 minutes around the coarse one.
                let (mut s_min, mut t_min, mut k_min) = (f64::MAX, bt, "");
                let mut k = bt - 3600.0;
                while k <= bt + 3600.0 {
                    if let (_, Some(e)) = detect_sky_events(k) {
                        if e.separation_deg < s_min {
                            (s_min, t_min, k_min) = (e.separation_deg, k, e.kind.label());
                        }
                    }
                    k += 300.0;
                }
                found.push((t_min, k_min));
                best = None;
            }
            (None, None) => {}
        }
        t += 1800.0;
    }
    found
}

fn hours_from_nasa(t: f64, nasa_t: f64) -> f64 {
    (t - nasa_t) / 3600.0
}

/// The page's eclipse seasons: sweeping 2026 and 2027, its detector finds
/// exactly NASA's four solar eclipses, each within 5 hours of NASA's greatest
/// eclipse, and the right kind where the model can tell. Measured 2026-10-21:
/// partial +2.4 h (2026-02-17), total -1.8 h, annular +1.2 h, total -0.0 h.
///
/// Red checks, run 2026-10-21, two faults stacked:
/// - the Moon's node and perigee held at J2000 (sol.json catalog_version 4)
///   and the centre-only disc test: one eclipse of four, "partial solar
///   eclipse -7.8 h from NASA's 2027-08-02", the Moon's eclipse seasons
///   about a month off;
/// - the rates in, the centre-only test kept: two of four, "partial +1.2 h"
///   (2027-02-06, annular) and "partial -0.0 h" (2027-08-02, total). The 2026
///   pair (gamma 0.90 and -0.97) never showed and both central ones were
///   called partial, because the test asked whether the discs overlap from
///   Earth's centre, where no one stands.
#[test]
fn the_maps_page_finds_nasas_solar_eclipses_of_2026_and_2027() {
    let found = swept_solar_eclipses(820_497_600.0, 883_569_600.0); // 2026-01-01 to 2028-01-01 UTC
    let shown: Vec<String> = found
        .iter()
        .map(|&(t, k)| {
            let near = NASA_SOLAR_ECLIPSES
                .iter()
                .min_by(|a, b| (a.1 - t).abs().total_cmp(&(b.1 - t).abs()))
                .expect("NASA rows");
            format!("{k} {:+.1} h from NASA's {}", hours_from_nasa(t, near.1), near.0)
        })
        .collect();
    println!("solar eclipses the Maps page sees in 2026-2027: {shown:#?}");
    assert_eq!(found.len(), NASA_SOLAR_ECLIPSES.len(), "the page's eclipses: {shown:#?}");
    for (&(when, nasa_t, nasa_kind), &(t, kind)) in NASA_SOLAR_ECLIPSES.iter().zip(&found) {
        let h = hours_from_nasa(t, nasa_t);
        assert!(
            h.abs() < ECLIPSE_HOURS && nasa_kind.map_or(true, |k| k == kind),
            "NASA's {when} ({nasa_kind:?}): the page has {kind} {h:+.1} h off ({shown:#?})"
        );
    }
}

/// What a person reads: the page's "upcoming" list, scanned from the day
/// before each of NASA's eclipses, names it, at its hour, as the kind NASA
/// does where the model can tell. This is the scan the page caches
/// (`scan_upcoming_sky_events`), with its own peak refinement. Measured
/// 2026-10-21: "partial solar eclipse (89% max coverage)" +2.4 h, "total solar
/// eclipse (100% max coverage)" -1.8 h, "annular solar eclipse (87% max
/// coverage)" +1.2 h, "total solar eclipse (100% max coverage)" +0.0 h.
///
/// Red checks, run 2026-10-21: with the elements held, none of the four was
/// listed (2027-08-02 came out as "Sun + Moon 0.37° (tight)": the centre-only
/// window was shorter than the scan's 3-hour step); with the rates but the
/// centre-only test, only 2027-08-02, as "partial solar eclipse (72% max
/// coverage)", and 2027-02-06 as "Sun + Moon 0.29° (tight)".
#[test]
fn the_upcoming_list_names_each_eclipse_at_its_hour() {
    let mut problems = Vec::new();
    for &(when, nasa_t, nasa_kind) in NASA_SOLAR_ECLIPSES {
        let list = scan_upcoming_sky_events(nasa_t - 86_400.0);
        let hit = list.iter().find(|e| {
            e.severity == 2
                && e.label.contains("solar eclipse")
                && hours_from_nasa(e.when_sim_seconds, nasa_t).abs() < ECLIPSE_HOURS
        });
        let line = match hit {
            Some(e) => format!("{when}: \"{}\" {:+.1} h", e.label, hours_from_nasa(e.when_sim_seconds, nasa_t)),
            None => format!("{when}: NOT LISTED (list: {:?})", list.iter().map(|e| &e.label).collect::<Vec<_>>()),
        };
        println!("{line}");
        let kind_ok = match (hit, nasa_kind) {
            (Some(e), Some(k)) => e.label.starts_with(k),
            (Some(_), None) => true,
            (None, _) => false,
        };
        if !kind_ok {
            problems.push(line);
        }
    }
    assert!(problems.is_empty(), "the upcoming list misses or misnames: {problems:#?}");
}

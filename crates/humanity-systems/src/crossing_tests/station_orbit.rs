//! From station/orbit.rs's tests (BUG-090): the home hangs over its longitude on every date, and
//! the deck sees the sun exactly when the simulation's clock (`time::local_hour`,
//! `solar::sun_factor`) says the sun is up.

use humanity_ship::station::orbit::*;
use glam::{DQuat, DVec3};

// As in station/orbit.rs's tests: the planet turns once a SOLAR day (the game day is a solar day by
// construction), and the tests run on the default game day of 24 hours.
const EARTH_ROT_S: f64 = REAL_SECONDS_PER_DAY;
const DAY: f64 = crate::time::EARTH_DAY_S;

/// station/orbit.rs's test orbit: synchronous, circular, equatorial, at the epoch.
fn geo_def() -> OrbitDef {
    OrbitDef {
        period: PeriodSpec::Synchronous,
        eccentricity: 0.0,
        inclination_deg: 0.0,
        raan_deg: 0.0,
        arg_periapsis_deg: 0.0,
        mean_anomaly_at_epoch_deg: 0.0,
        epoch_game_seconds: 0.0,
    }
}

/// BUG-090: the home hangs over its own longitude on EVERY date, and the
/// deck (LVLH, floor to the planet) has the sun above it exactly when the
/// home's clock says the sun is up. The dates are the sun's azimuth, which
/// creeps a full turn a year; before the fix the longitude below the home
/// was `-122.3 - azimuth + 180`, right only when the azimuth sat near 180
/// degrees (late September). Red check, run: propagating `def` itself
/// instead of `over_its_longitude(&def, ..)` fails at azimuth 0.
#[test]
fn a_synchronous_station_hangs_over_its_longitude_on_every_date() {
    let mut def = geo_def();
    def.mean_anomaly_at_epoch_deg = -122.3;
    let wrap = |d: f64| (d + 180.0).rem_euclid(360.0) - 180.0;
    for sun_az_deg in [0.0_f64, 93.0, 183.0, 271.0] {
        let sun_az = sun_az_deg.to_radians();
        let d = over_its_longitude(&def, sun_az);
        for day in [0.0_f64, 3.0, 200.0] {
            for hour in [0.0_f64, 5.5, 9.0, 12.0, 17.25, 20.15, 23.9] {
                let t_game = (day * 24.0 + hour) * 3600.0;
                let (pos, _) = propagate(&d, MU_EARTH, EARTH_ROT_S, sim_seconds(t_game, DAY), DAY);
                let spin = humanity_core::dev_travel::planet_spin_from_time(hour, sun_az);
                let fixed = DQuat::from_rotation_y(-spin) * pos;
                let lon = (-fixed.z).atan2(fixed.x).to_degrees();
                assert!(
                    wrap(lon - -122.3).abs() < 1.0e-6,
                    "azimuth {sun_az_deg}, day {day}, hour {hour}: the home hangs over {lon}, not -122.3"
                );
                // Eternal equinox: the sun lies in the equator plane.
                let sun = DVec3::new(sun_az.cos(), 0.0, -sun_az.sin());
                let above = pos.normalize().dot(sun);
                let home_hour = crate::time::local_hour(hour, -122.3, 24) as f32;
                let clock_up = crate::solar::sun_factor(home_hour) > 0.0;
                if above.abs() > 0.01 {
                    assert_eq!(
                        above > 0.0,
                        clock_up,
                        "azimuth {sun_az_deg}, hour {hour}: the deck sees the sun {above:.3}, the home clock reads {home_hour}"
                    );
                }
            }
        }
    }
    assert!((hang_longitude_deg(&def, DAY).unwrap() - -122.3).abs() < 1.0e-9);
    // An epoch quoted at 06:00 has turned the planet a quarter under it.
    let mut later = def.clone();
    later.epoch_game_seconds = 6.0 * 3600.0;
    assert!(wrap(hang_longitude_deg(&later, DAY).unwrap() - (-122.3 - 90.0)).abs() < 1.0e-9);
    let leo = OrbitDef { period: PeriodSpec::Seconds(5545.0), ..def.clone() };
    assert_eq!(over_its_longitude(&leo, 1.0), leo, "only a synchronous orbit is re-phased");
    assert_eq!(hang_longitude_deg(&leo, DAY), None);
}

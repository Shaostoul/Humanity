//! TESTS OF THE CORE, TOGETHER WITH TERRAIN (the crate split, 2026-10-31,
//! docs/design/crate-split.md): written beside the core's code, they reach into this crate to do it,
//! so each moved up WHOLE to the lowest crate that may use everything it reaches. Nothing in a test
//! changed but its paths. From dev_travel.rs: the sun and the moon over the ground, placed by the
//! heightmap's latitude and longitude.

use crate::dev_travel::*;
use crate::dev_travel::tests::model_sun_from_earth;
use glam::DVec3;

/// Where a world direction stands over the spinning planet at game hour
/// `hour` (the ground point it is overhead of), through the game's own
/// chain: the spin from the sun's azimuth, then `Ry(-spin)` into the
/// planet's frame, then the terrain's latitude and longitude.
fn ground_point(dir: DVec3, sun: DVec3, hour: f64) -> (f64, f64) {
    let spin = planet_spin_from_time(hour, (-sun.z).atan2(sun.x));
    crate::terrain::planet_heightmap::dir_to_latlon_deg_f64(glam::DQuat::from_rotation_y(-spin) * dir)
}

/// The sun-frame relation (v0.878), locked against the REAL longitude
/// convention: at game hour H the subsolar longitude must be (12-H)*15
/// degrees. Composes planet_spin_from_time with the same Ry rotation the
/// render applies and the terrain's dir_to_latlon convention, so a
/// handedness change anywhere breaks HERE, not in a night-time surprise.
#[test]
fn spin_puts_the_subsolar_longitude_where_the_clock_says() {
    // A sun direction with nonzero azimuth so phase errors can't hide.
    let sun_world = DVec3::new(0.6, 0.2, 0.75).normalize();
    let sun_az = (-sun_world.z).atan2(sun_world.x);
    for (hour, want_lon_deg) in [
        (12.0, 0.0),
        (18.0, -90.0), // evening at lon 0: subsolar has moved west
        (6.0, 90.0),   // morning at lon 0: subsolar still to the east
        (15.0, -45.0),
    ] {
        let spin = planet_spin_from_time(hour, sun_az);
        // Subsolar direction in the PLANET-LOCAL frame.
        let local = glam::DQuat::from_rotation_y(-spin) * sun_world;
        let (_, lon) = crate::terrain::planet_heightmap::dir_to_latlon_deg(
            glam::Vec3::new(local.x as f32, local.y as f32, local.z as f32),
        );
        let diff = (lon as f64 - want_lon_deg).rem_euclid(360.0);
        let diff = diff.min(360.0 - diff);
        assert!(
            diff < 0.01,
            "hour {hour}: subsolar lon {lon} (want {want_lon_deg})"
        );
    }
    // Continuity: one game second of a 24-hour day advances the spin by
    // TAU/86400.
    let a = planet_spin_from_time(10.0, sun_az);
    let b = planet_spin_from_time(10.0 + 24.0 / 86_400.0, sun_az);
    let step = (b - a).rem_euclid(std::f64::consts::TAU);
    assert!(
        (step - std::f64::consts::TAU / 86_400.0).abs() < 1e-12,
        "spin rate must be one turn per game day"
    );
}

/// The sun-clock relation (2026-08-18 incident): the game clock is
/// LON-0 mean solar time, so the local hour at a site is
/// hour + lon/15 - and the ipc camera "time" pin converts the other
/// way (global = local - lon/15). This test drives the FULL chain
/// (latlon_to_dir -> Ry(planet_spin_from_time) -> dot with the sun)
/// at 4 latitudes x 4 local hours and demands the astronomical
/// elevation. Feeding the local hour straight in as global (the old
/// HUD/pin behavior) fails the Silverdale-noon cell by ~62 degrees -
/// the test is unfakeable. Tolerance 2 degrees is headroom for a
/// future axial-tilt term; today the model is eternal equinox
/// (dec = 0: sol.json Earth inclination 0, spin a bare Ry).
#[test]
fn local_solar_time_puts_the_sun_at_the_astronomical_elevation() {
    // Sun in the model's ecliptic plane (Y = 0), nonzero azimuth.
    let sun_world = DVec3::new(0.6, 0.0, 0.75).normalize();
    let sun_az = (-sun_world.z).atan2(sun_world.x);
    for (lat, lon) in [
        (47.645f64, -122.6925f64), // Silverdale (the incident site)
        (0.0, 0.0),                // null island
        (-33.9, 151.2),            // Sydney
        (65.0, -18.4),             // Iceland
    ] {
        for t_local in [6.0f64, 9.0, 12.0, 16.0] {
            // The ipc conversion under test.
            let h_global = (t_local - lon / 15.0).rem_euclid(24.0);
            let spin = planet_spin_from_time(h_global, sun_az);
            let d = crate::terrain::planet_heightmap::latlon_to_dir(
                lat as f32, lon as f32,
            );
            let up = glam::DQuat::from_rotation_y(spin)
                * DVec3::new(d.x as f64, d.y as f64, d.z as f64);
            let el_engine =
                up.normalize().dot(sun_world).asin().to_degrees();
            // Astronomical elevation at declination 0:
            // sin(el) = cos(lat) * cos(hour_angle).
            let ha = ((t_local - 12.0) * 15.0f64).to_radians();
            let el_expected =
                (lat.to_radians().cos() * ha.cos()).asin().to_degrees();
            assert!(
                (el_engine - el_expected).abs() < 2.0,
                "lat {lat} lon {lon} local {t_local}: \
                 engine {el_engine:.1} vs astronomical {el_expected:.1}"
            );
        }
    }
}

/// BUG-140 seen from the ground: the Moon crosses an observer's meridian
/// at the hour its phase says, a new Moon at noon, a first quarter at
/// 18:00 (high in the south at dusk), a full Moon at midnight and a last
/// quarter at 06:00. The phase instants are the US Naval Observatory's
/// (https://aa.usno.navy.mil/api/moon/phases/date?date=2026-09-01&nump=12,
/// fetched 2026-10-21, UT), for an observer at longitude 0, so the game
/// clock is their local time. The Moon must stand within 2.5 degrees of
/// the meridian (the model's few-degree lunar error; see
/// `cosmos::tests::the_moon_keeps_the_naval_observatory_phases`). Red
/// check, run: under the mirror a first-quarter Moon crossed at 06:00.
#[test]
fn the_moon_crosses_the_meridian_at_the_hour_its_phase_says() {
    let mut table = String::new();
    let mut worst = 0.0_f64;
    for (when, what, t, transit_hour) in [
        ("2026-10-10 15:50", "New Moon", 844_919_400.0, 12.0),
        ("2026-10-18 16:12", "First Quarter", 845_611_920.0, 18.0),
        ("2026-10-26 04:12", "Full Moon", 846_259_920.0, 0.0),
        ("2026-11-01 20:28", "Last Quarter", 846_836_880.0, 6.0),
    ] {
        let at = |id: &str| {
            crate::cosmos::body_world_position_3d_au(crate::cosmos::find_body(id).expect(id), t)
        };
        let moon = (at("moon") - at("earth")).normalize();
        let sun = model_sun_from_earth(t);
        let (_, lon) = ground_point(moon, sun, transit_hour);
        worst = worst.max(lon.abs());
        table.push_str(&format!("\n  {when} {what:<13} at {transit_hour:4.1} h the Moon is over longitude {lon:8.2}"));
    }
    println!("the Moon over the ground at its phase's transit hour:{table}");
    assert!(worst < 2.5, "the Moon is not on the meridian at its phase's hour:{table}");
}

/// BUG-140 (2026-10-21): turning the ecliptic into the world by a proper
/// rotation instead of a mirror must not move the Sun on the ground,
/// because the spin is DEFINED from the sun's azimuth. The orbit model's
/// Sun is taken as it is and as the other mapping would place it (the
/// mirror and the rotation differ only in the sign of world Z), on four
/// dates and six game hours, and the ground track must be the same to
/// 1e-9 degree and sit where the clock says, `(12 - hour) * 15` degrees.
/// Prints the track, so a run before and a run after the fix can be
/// compared line by line.
#[test]
fn the_suns_ground_track_does_not_depend_on_the_ecliptic_mapping() {
    for (date, t) in [
        ("2026-03-20 00:00", 827_236_800.0),
        ("2026-06-21 12:00", 835_315_200.0),
        ("2026-10-21 00:00", 845_812_800.0),
        ("2026-12-21 18:00", 851_148_000.0),
    ] {
        let sun = model_sun_from_earth(t);
        let other = DVec3::new(sun.x, sun.y, -sun.z);
        for hour in [0.0, 6.0, 9.5, 12.0, 18.0, 20.2] {
            let (lat, lon) = ground_point(sun, sun, hour);
            let (lat_o, lon_o) = ground_point(other, other, hour);
            println!("sun over the ground {date} hour {hour:4.1}: lat {lat:.6} lon {lon:.6}");
            let same = (lat - lat_o).abs() + ((lon - lon_o + 540.0).rem_euclid(360.0) - 180.0).abs();
            assert!(same < 1e-9, "{date} hour {hour}: the two mappings disagree ({lat}, {lon}) vs ({lat_o}, {lon_o})");
            let want = (12.0 - hour) * 15.0;
            let off = (lon - want + 540.0).rem_euclid(360.0) - 180.0;
            assert!(off.abs() < 1e-9, "{date} hour {hour}: the sun stands over {lon}, the clock says {want}");
            assert!(lat.abs() < 0.01, "{date}: the eternal equinox keeps the sun on the equator, got {lat}");
        }
    }
}

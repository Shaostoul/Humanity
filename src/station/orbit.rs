//! Orbital state for crewed stations: where the homestead IS, and which way it
//! is POINTING, as a pure function of the game clock.
//!
//! ## Why this module exists (2026-08-26)
//!
//! The operator, after the F11 time-of-day slider shipped: *"it's turning the
//! planet but, I can't get it to affect the homestead locked in orbit with the
//! Earth. It seems like I may need to add a control to the mothership to
//! actually change its orientation to also change that of the lighting."*
//!
//! That instinct is exactly right, and the arithmetic says it is the ONLY thing
//! that can work. Sun direction is computed as `sun_world - ship_world`, and the
//! sun is 1 AU away. Sweeping the station around a full geosynchronous circle
//! moves that direction by
//!
//! ```text
//! 2 * 4.2164e7 / 1.496e11 = 5.6e-4 rad = 0.032 degrees
//! ```
//!
//! Thirty thousandths of a degree, over an entire orbit. **Orbital position can
//! never light a hull differently.** Only the hull's ORIENTATION against a
//! effectively-fixed sun can, which is why the old station - a bare
//! `station_world_pos: DVec3` with no attitude anywhere in the codebase - sat in
//! permanent noon no matter what the clock did.
//!
//! Ground sites work the other way around for the same reason: the planet's spin
//! sweeps a surface site's position so its radial `up` turns under a fixed sun.
//! A station hull has no radial that turns unless it is given one.
//!
//! ## What a real station does
//!
//! Crewed stations fly **LVLH** (local-vertical/local-horizontal, "nadir
//! pointing"): the floor faces the planet and the long axis runs along-track.
//! That is not a rendering convenience, it is how the ISS and every serious
//! habitat design flies, because the crew needs a consistent down and the
//! windows need to look at the planet. Under LVLH the body frame rotates once
//! per orbit against the inertial sun, so **the sun sweeps the full 360 degrees
//! around the hull once per orbital period with no lighting special-case at
//! all**. Put the station at a synchronous radius and that period is one day,
//! and the hour slider means the same thing aboard as it does on the ground.
//!
//! ## The unit trap
//!
//! `GameTime::elapsed_seconds` counts GAME seconds, and a game day is
//! `GameTime::seconds_per_day` of them: 86,400 on the default 24-hour day, but
//! the day length is a setting (12 to 48 hours, the one clock of 2026-09-27).
//! One game day is one turn of the planet whatever its length, so every
//! celestial rate is scaled by 86,400 over the day's seconds. [`sim_seconds`]
//! is the conversion and it has its own test, because getting it wrong
//! produces an orbit that looks plausible and is off by the day's ratio.

use glam::{DMat3, DQuat, DVec3};

/// Earth's gravitational parameter, m^3/s^2 (WGS-84).
pub const MU_EARTH: f64 = 3.986_004_418e14;

/// Seconds in a real solar day. The bridge between physical orbital mechanics
/// and the compressed game clock.
pub const REAL_SECONDS_PER_DAY: f64 = 86_400.0;

/// Convert game seconds to the physical seconds an orbit propagator wants.
///
/// A game day of `game_day_s` seconds is one turn of the planet, so every
/// celestial rate is scaled by the same factor or the sky and the station
/// disagree about what "a day" is. On the default 24-hour day the factor is
/// 1; on a 36-hour day it is 2/3.
pub fn sim_seconds(game_elapsed_s: f64, game_day_s: f64) -> f64 {
    game_elapsed_s * (REAL_SECONDS_PER_DAY / game_day_s.max(1.0))
}

/// How the orbit's size is specified.
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum PeriodSpec {
    /// Match the parent's rotation period, i.e. hang over one spot on the
    /// surface. "Geostationary" is this plus zero inclination; it is a real
    /// orbit design, not a graphics shortcut, which is why it is expressed as a
    /// period rather than a magic altitude.
    Synchronous,
    /// An explicit orbital period in PHYSICAL seconds (5545 s is a 400 km LEO).
    Seconds(f64),
    /// An explicit semi-major axis in metres, measured from the parent's centre.
    SemiMajorAxisM(f64),
}

/// Keplerian elements, in the parent-centred inertial frame.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct OrbitDef {
    pub period: PeriodSpec,
    #[serde(default)]
    pub eccentricity: f64,
    #[serde(default)]
    pub inclination_deg: f64,
    #[serde(default)]
    pub raan_deg: f64,
    #[serde(default)]
    pub arg_periapsis_deg: f64,
    /// Where in the orbit the station sits at the epoch. For a synchronous
    /// equatorial orbit this is effectively the longitude it hangs over.
    #[serde(default)]
    pub mean_anomaly_at_epoch_deg: f64,
    /// The game-clock reading the elements are quoted at.
    #[serde(default)]
    pub epoch_game_seconds: f64,
}

/// How the hull is pointed.
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum AttitudeMode {
    /// Nadir pointing: floor toward the planet, long axis along-track. The way
    /// real stations fly, and the mode that makes the sun sweep the hull.
    Lvlh {
        yaw_deg: f64,
        pitch_deg: f64,
        roll_deg: f64,
    },
    /// Hold a fixed orientation in the inertial frame. The sun then never moves
    /// relative to the hull, which is the OLD behaviour: kept as an explicit,
    /// named choice rather than an accident.
    Inertial {
        yaw_deg: f64,
        pitch_deg: f64,
        roll_deg: f64,
    },
}

impl Default for AttitudeMode {
    fn default() -> Self {
        AttitudeMode::Lvlh {
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            roll_deg: 0.0,
        }
    }
}

/// One station, as loaded from `data/stations/*.ron`.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct StationDef {
    pub id: String,
    pub name: String,
    /// Body id this station orbits, e.g. "earth".
    pub parent: String,
    pub orbit: OrbitDef,
    #[serde(default)]
    pub attitude: AttitudeMode,
}

/// Semi-major axis in metres for this orbit around a parent with the given mu
/// and rotation period (seconds, physical).
pub fn semi_major_axis_m(def: &OrbitDef, mu: f64, parent_rotation_s: f64) -> f64 {
    match def.period {
        PeriodSpec::SemiMajorAxisM(a) => a.max(1.0),
        PeriodSpec::Seconds(t) => axis_from_period(t.max(1.0), mu),
        PeriodSpec::Synchronous => axis_from_period(parent_rotation_s.max(1.0), mu),
    }
}

/// Kepler's third law, solved for a: `a = (mu * (T / 2pi)^2)^(1/3)`.
pub fn axis_from_period(period_s: f64, mu: f64) -> f64 {
    let n = std::f64::consts::TAU / period_s;
    (mu / (n * n)).cbrt()
}

/// Orbital period in physical seconds for a given semi-major axis.
pub fn period_from_axis(a_m: f64, mu: f64) -> f64 {
    std::f64::consts::TAU * (a_m * a_m * a_m / mu).sqrt()
}

/// Propagate to a parent-centred inertial position and velocity.
///
/// `t_sim_s` is PHYSICAL seconds (run game seconds through [`sim_seconds`]
/// first). The 3-1-3 `Rz(raan) * Rx(incl) * Rz(argp)` composition matches
/// `cosmos::body_position_relative_au`, deliberately: a handedness mistake then
/// shows up in one convention rather than two that disagree.
pub fn propagate(
    def: &OrbitDef,
    mu: f64,
    parent_rotation_s: f64,
    t_sim_s: f64,
    game_day_s: f64,
) -> (DVec3, DVec3) {
    let a = semi_major_axis_m(def, mu, parent_rotation_s);
    let e = def.eccentricity.clamp(0.0, 0.95);
    let n = (mu / (a * a * a)).sqrt();
    let dt = t_sim_s - sim_seconds(def.epoch_game_seconds, game_day_s);
    let m = def.mean_anomaly_at_epoch_deg.to_radians() + n * dt;
    let ea = crate::cosmos::kepler_solve(m.rem_euclid(std::f64::consts::TAU), e);

    // Perifocal frame.
    let (se, ce) = ea.sin_cos();
    let p = DVec3::new(a * (ce - e), a * (1.0 - e * e).sqrt() * se, 0.0);
    // Time derivative of the above, via dE/dt = n / (1 - e cos E).
    let edot = n / (1.0 - e * ce);
    let v = DVec3::new(
        -a * se * edot,
        a * (1.0 - e * e).sqrt() * ce * edot,
        0.0,
    );

    let rot = DMat3::from_rotation_z(def.raan_deg.to_radians())
        * DMat3::from_rotation_x(def.inclination_deg.to_radians())
        * DMat3::from_rotation_z(def.arg_periapsis_deg.to_radians());

    // The engine's world frame is Y-up while the orbital plane above is built
    // in a Z-up convention (the standard one for these elements), so swing the
    // plane into engine axes: orbital +Z (angular momentum) becomes engine +Y.
    let to_engine = DMat3::from_rotation_x(-std::f64::consts::FRAC_PI_2);
    (to_engine * (rot * p), to_engine * (rot * v))
}

/// The elements to propagate so a SYNCHRONOUS orbit hangs over its own
/// longitude on EVERY date (BUG-090, 2026-09-28). Other orbits pass through
/// unchanged.
///
/// Why it is needed: the planet's spin is tied to the SUN
/// (`dev_travel::planet_spin_from_time` is `sun_azimuth + (hour - 12) *
/// TAU / 24`), and the sun's azimuth creeps round a full turn each year, while
/// `propagate` places the station by the game clock alone. So the longitude
/// the home hung over was `mean_anomaly_at_epoch - sun_azimuth + 180 deg`:
/// right in late September, when the sun's azimuth is near 180 degrees, and a
/// full turn of the planet out by the next September. The deck's noon drifted
/// through every hour of the game clock over a year, while the crops, the
/// panels and the HUD kept the clock's noon.
///
/// Adding `sun_azimuth - 180 deg` to the mean anomaly makes the station's
/// inertial longitude `L + spin`, which is the definition of hanging over
/// longitude `L`: `mean_anomaly_at_epoch_deg` is then exactly the longitude
/// below the home (for the home's `epoch_game_seconds` of 0, or any midnight),
/// and the deck's local solar time is the game clock's plus `L / 15` hours.
/// `sun_azimuth_rad` is `engine::frame_lock::sun_azimuth`, the same number
/// the spin uses, so the two cannot disagree.
pub fn over_its_longitude(def: &OrbitDef, sun_azimuth_rad: f64) -> OrbitDef {
    let mut d = def.clone();
    if d.period == PeriodSpec::Synchronous {
        d.mean_anomaly_at_epoch_deg += (sun_azimuth_rad - std::f64::consts::PI).to_degrees();
    }
    d
}

/// The longitude a synchronous orbit hangs over (degrees, east positive),
/// once propagated through [`over_its_longitude`]. None for any other orbit,
/// which has no fixed place below it.
pub fn hang_longitude_deg(def: &OrbitDef, game_day_s: f64) -> Option<f64> {
    if def.period != PeriodSpec::Synchronous {
        return None;
    }
    // The epoch's own hour turns the planet under the quoted phase.
    let epoch_turns = def.epoch_game_seconds.rem_euclid(game_day_s.max(1.0)) / game_day_s.max(1.0);
    let lon = def.mean_anomaly_at_epoch_deg - 360.0 * epoch_turns;
    Some((lon + 180.0).rem_euclid(360.0) - 180.0)
}

/// The body-to-world rotation for a station at `pos` moving at `vel`, relative
/// to a parent centred at the origin of the same frame.
///
/// Under [`AttitudeMode::Lvlh`] the returned quaternion maps body `+Y` onto the
/// local zenith (so body `-Y`, the floor, faces the planet) and body `+Z` onto
/// the along-track direction.
pub fn attitude(mode: &AttitudeMode, pos: DVec3, vel: DVec3) -> DQuat {
    let (yaw, pitch, roll, base) = match *mode {
        AttitudeMode::Inertial {
            yaw_deg,
            pitch_deg,
            roll_deg,
        } => (yaw_deg, pitch_deg, roll_deg, DQuat::IDENTITY),
        AttitudeMode::Lvlh {
            yaw_deg,
            pitch_deg,
            roll_deg,
        } => {
            let up = pos.normalize_or_zero();
            if up.length_squared() < 0.5 {
                (yaw_deg, pitch_deg, roll_deg, DQuat::IDENTITY)
            } else {
                // Along-track, with any radial component removed so the basis
                // is exactly orthonormal even for a slightly eccentric orbit.
                let mut fwd = vel - up * vel.dot(up);
                if fwd.length_squared() < 1.0e-12 {
                    // Degenerate (zero velocity): any perpendicular will do.
                    fwd = up.cross(DVec3::X);
                    if fwd.length_squared() < 1.0e-12 {
                        fwd = up.cross(DVec3::Z);
                    }
                }
                let fwd = fwd.normalize();
                // Right-handed basis with X = Y cross Z.
                let right = up.cross(fwd);
                (
                    yaw_deg,
                    pitch_deg,
                    roll_deg,
                    DQuat::from_mat3(&DMat3::from_cols(right, up, fwd)),
                )
            }
        }
    };
    base * DQuat::from_euler(
        glam::EulerRot::YXZ,
        yaw.to_radians(),
        pitch.to_radians(),
        roll.to_radians(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // The SIDEREAL day, for checking the Kepler math against the textbook
    // geostationary radius.
    const EARTH_SIDEREAL_S: f64 = 86_164.0905;
    // What the ENGINE passes, and the distinction matters by 362 km of orbit
    // radius. Real geostationary is synchronous with the sidereal day because
    // Earth turns relative to the stars. This game defines its day relative to
    // the SUN by construction - planet_spin_from_time is literally
    // sun_azimuth + (hour - 12) * TAU / 24 - so the game day IS a solar day,
    // and a station that is to hang over one spot in THIS sky must be
    // synchronous with that. Using the sidereal figure here would leave the
    // home drifting a quarter degree of longitude per game day.
    const EARTH_ROT_S: f64 = REAL_SECONDS_PER_DAY;
    /// The game day the tests run on: the default 24 hours.
    const DAY: f64 = crate::systems::time::EARTH_DAY_S;

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

    /// The unit trap, guarded: one game day is one turn of the planet at any
    /// day length. Red check, run: `sim_seconds` as the identity passes the
    /// 24-hour day and fails the 36-hour one (one game day would be 1.5 real
    /// days of orbit).
    #[test]
    fn sim_seconds_makes_one_game_day_one_turn_at_any_day_length() {
        for hours in [24.0, 36.0, 12.0] {
            let day = hours * 3600.0;
            let phys = sim_seconds(day, day);
            assert!(
                (phys - REAL_SECONDS_PER_DAY).abs() < 1.0e-6,
                "{hours}-hour day: one game day must be one real day of orbital motion, got {phys} s"
            );
        }
        assert!((sim_seconds(1.0, 86_400.0) - 1.0).abs() < 1.0e-12, "1:1 on the default day");
    }

    /// A synchronous orbit must come out at the textbook geostationary radius.
    #[test]
    fn synchronous_orbit_lands_at_the_geostationary_radius() {
        let a = semi_major_axis_m(&geo_def(), MU_EARTH, EARTH_SIDEREAL_S);
        // 42,164 km from Earth's centre.
        assert!(
            (a - 4.2164e7).abs() < 2.0e4,
            "expected ~42164 km, got {:.1} km",
            a / 1000.0
        );
    }

    /// The floor must face the planet at every point of the orbit. This is the
    /// one that catches a basis mix-up, which otherwise looks fine head-on and
    /// wrong a quarter orbit later.
    #[test]
    fn lvlh_floor_points_at_the_parent() {
        let def = geo_def();
        let mode = AttitudeMode::default();
        for i in 0..16 {
            let t = sim_seconds(i as f64 * DAY / 16.0, DAY);
            let (pos, vel) = propagate(&def, MU_EARTH, EARTH_ROT_S, t, DAY);
            let q = attitude(&mode, pos, vel);
            let floor = q * DVec3::NEG_Y;
            let nadir = -pos.normalize();
            let ang = floor.dot(nadir).clamp(-1.0, 1.0).acos().to_degrees();
            assert!(
                ang < 0.1,
                "sample {i}: floor is {ang:.3} deg off nadir, must point at the planet"
            );
        }
    }

    /// Half a game day must put a synchronous station on the far side. This is
    /// the test that specifically guards the GAME-CLOCK coupling: it fails
    /// outright if anyone reintroduces a wall-clock phase.
    #[test]
    fn scrubbing_twelve_hours_puts_the_synchronous_station_antipodal() {
        let def = geo_def();
        let day = DAY;
        let (a_pos, _) = propagate(&def, MU_EARTH, EARTH_ROT_S, sim_seconds(0.0, day), day);
        let (b_pos, _) = propagate(&def, MU_EARTH, EARTH_ROT_S, sim_seconds(day * 0.5, day), day);
        let sum = (a_pos + b_pos).length();
        assert!(
            sum < 5.0e4,
            "12 h apart should be antipodal (sum ~0), got |a+b| = {:.1} km",
            sum / 1000.0
        );
    }

    /// THE LOAD-BEARING ONE. Drive the whole chain the engine drives - propagate,
    /// attitude, then rotate the world sun into the hull frame - and demand the
    /// sun's elevation above the DECK track the hour the way it does for an
    /// equatorial ground site.
    ///
    /// It cannot be faked by echoing the implementation, because the expected
    /// value comes from the astronomical hour-angle formula, not from the code
    /// under test. On the pre-fix engine the hull elevation was constant, so
    /// this fails by up to 90 degrees.
    #[test]
    fn hull_sun_elevation_tracks_the_clock() {
        let def = geo_def();
        let mode = AttitudeMode::default();
        // Sun parked along +X at 1 AU in the same inertial frame. At t=0 the
        // station's mean anomaly is 0, which also puts it on +X: local noon.
        let sun_world = DVec3::new(1.496e11, 0.0, 0.0);
        let day = DAY;
        for i in 0..8 {
            let frac = i as f64 / 8.0;
            let t_game = frac * day;
            let (pos, vel) = propagate(&def, MU_EARTH, EARTH_ROT_S, sim_seconds(t_game, day), day);
            let q = attitude(&mode, pos, vel);
            let sun_dir_world = (sun_world - pos).normalize();
            // Into the hull frame, exactly as the renderer does it.
            let sun_body = q.inverse() * sun_dir_world;
            let elev = sun_body.dot(DVec3::Y).clamp(-1.0, 1.0).asin().to_degrees();
            // Astronomical answer: hour angle from local noon at t=0.
            // Astronomical answer: the sun sits at the zenith at t=0 and the
            // station turns once per day under LVLH, so its elevation above
            // the deck is asin(cos(hour angle)) - the same shape a ground site
            // on the equator sees. Derived from the geometry, NOT from the code
            // under test, which is what stops this being a tautology.
            let hour_angle = frac * std::f64::consts::TAU;
            let expect = hour_angle.cos().clamp(-1.0, 1.0).asin().to_degrees();
            assert!(
                (elev - expect).abs() < 2.0,
                "t={frac:.3} day: hull sun elevation {elev:.2} deg, astronomical {expect:.2} deg"
            );
        }
    }

    /// Inertial attitude is the OLD behaviour, and must still be available as a
    /// deliberate choice: the sun does not move relative to the hull.
    #[test]
    fn inertial_attitude_keeps_the_sun_fixed_on_the_hull() {
        let def = geo_def();
        let mode = AttitudeMode::Inertial {
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            roll_deg: 0.0,
        };
        let sun_world = DVec3::new(1.496e11, 0.0, 0.0);
        let day = DAY;
        let mut first = None;
        for i in 0..8 {
            let t = sim_seconds(i as f64 * day / 8.0, day);
            let (pos, vel) = propagate(&def, MU_EARTH, EARTH_ROT_S, t, DAY);
            let q = attitude(&mode, pos, vel);
            let sun_body = q.inverse() * (sun_world - pos).normalize();
            let elev = sun_body.dot(DVec3::Y).clamp(-1.0, 1.0).asin().to_degrees();
            match first {
                None => first = Some(elev),
                Some(f) => assert!(
                    (elev - f).abs() < 0.05,
                    "inertial hold should not move the sun on the hull: {elev} vs {f}"
                ),
            }
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
                    let spin = crate::dev_travel::planet_spin_from_time(hour, sun_az);
                    let fixed = DQuat::from_rotation_y(-spin) * pos;
                    let lon = (-fixed.z).atan2(fixed.x).to_degrees();
                    assert!(
                        wrap(lon - -122.3).abs() < 1.0e-6,
                        "azimuth {sun_az_deg}, day {day}, hour {hour}: the home hangs over {lon}, not -122.3"
                    );
                    // Eternal equinox: the sun lies in the equator plane.
                    let sun = DVec3::new(sun_az.cos(), 0.0, -sun_az.sin());
                    let above = pos.normalize().dot(sun);
                    let home_hour = crate::systems::time::local_hour(hour, -122.3, 24) as f32;
                    let clock_up = crate::systems::solar::sun_factor(home_hour) > 0.0;
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
}

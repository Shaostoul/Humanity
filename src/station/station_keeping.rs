//! Station-keeping: what it costs to stay where you parked, and what happens
//! when you stop paying.
//!
//! `orbit.rs` answers "where is the station at time t" for a FIXED orbit. This
//! module answers the questions that make parking a decision:
//!
//! - How fast does this orbit decay with the engines cut?
//! - How long until that decay ends in the atmosphere?
//! - What does holding position cost per year?
//!
//! ## Why the parking spot is a real choice
//!
//! Every candidate has a different failure mode and a wildly different clock,
//! and all of it is honest physics rather than tuning:
//!
//! | Where | Failure with no power | Clock |
//! |---|---|---|
//! | Low orbit | drag decay, then reentry | hours to weeks |
//! | Geostationary | inclination and longitude drift | years, never fatal |
//! | Collinear Lagrange (L1/L2) | exponential drift off-station | days |
//! | Triangular Lagrange (L4/L5) | none, they are stable | forever |
//!
//! So "lose power and crash" is a LOW-ORBIT story. It cannot happen at
//! geostationary, and at L1 the ship does not crash, it wanders off. A fleet
//! that wants the cheap ride down to Earth accepts a decay clock; one that
//! wants safety parks high and gives up the access.
//!
//! ## Pure, like the rest of the orbit code
//!
//! No ECS, no globals, no clock. Everything here is a function of its inputs so
//! it can be checked headlessly, the same discipline as `orbit::propagate` and
//! `surface_move::radial_step`.

/// Earth's gravitational parameter, m^3/s^2.
pub const MU_EARTH: f64 = 3.986_004_418e14;
/// Earth's equatorial radius, m.
pub const R_EARTH: f64 = 6_378_137.0;
/// Altitude at which a decaying orbit is considered lost, m. Real reentry
/// interface is conventionally 120 km: below it drag rises fast enough that
/// nothing recovers.
pub const REENTRY_ALTITUDE_M: f64 = 120_000.0;

/// One layer of a piecewise-exponential atmosphere: base altitude (km),
/// density there (kg/m^3), and the scale height (km) above it.
///
/// INFINITE-OF-X DEBT, logged rather than accepted silently: this table is
/// EARTH's, hardcoded. Atmospheres exist more than once, so per-planet density
/// belongs in data. The pattern to mirror already exists and is proven:
/// `PlanetDef::gravity_curve` (`src/terrain/planet.rs:176`) is an authored
/// `Vec<(altitude, value)>` with a clamped lookup (`gravity_at`) and a
/// load-time sanitizer (`normalize_gravity_curve`). A `density_curve` should be
/// its near-exact twin, and `data/solar_system/earth.ron` already authors
/// `scale_height_km: 8.5` to seed it.
///
/// Kept in code for now because these are published US-Standard-Atmosphere
/// reference values rather than authored content, and because only Earth has an
/// orbit anyone can decay from today. Move it the moment a second body needs one.
///
/// These are the standard US-Standard/Jacchia-style tabulated values for
/// moderate solar activity. Real thermospheric density swings by more than an
/// order of magnitude with the solar cycle, which is exactly why real operators
/// cannot predict a reentry date precisely, and why a decay clock in this game
/// should be presented as an estimate rather than a countdown.
const DENSITY_LAYERS: &[(f64, f64, f64)] = &[
    (0.0, 1.225, 7.249),
    (25.0, 3.899e-2, 6.349),
    (30.0, 1.774e-2, 6.682),
    (40.0, 3.972e-3, 7.554),
    (50.0, 1.057e-3, 8.382),
    (60.0, 3.206e-4, 7.714),
    (70.0, 8.770e-5, 6.549),
    (80.0, 1.905e-5, 5.799),
    (90.0, 3.396e-6, 5.382),
    (100.0, 5.297e-7, 5.877),
    (110.0, 9.661e-8, 7.263),
    (120.0, 2.438e-8, 9.473),
    (130.0, 8.484e-9, 12.636),
    (140.0, 3.845e-9, 16.149),
    (150.0, 2.070e-9, 22.523),
    (180.0, 5.464e-10, 29.740),
    (200.0, 2.789e-10, 37.105),
    (250.0, 7.248e-11, 45.546),
    (300.0, 2.418e-11, 53.628),
    (350.0, 9.518e-12, 53.298),
    (400.0, 3.725e-12, 58.515),
    (450.0, 1.585e-12, 60.828),
    (500.0, 6.967e-13, 63.822),
    (600.0, 1.454e-13, 71.835),
    (700.0, 3.614e-14, 88.667),
    (800.0, 1.170e-14, 124.64),
    (900.0, 5.245e-15, 181.05),
    (1000.0, 3.019e-15, 268.00),
];

/// Atmospheric density (kg/m^3) at a geometric altitude in metres.
///
/// Piecewise exponential: within each tabulated band, density falls off with
/// that band's scale height. Above the table it keeps falling with the last
/// scale height, which is physically reasonable and, more importantly, never
/// returns zero, so a very high orbit decays immeasurably slowly instead of not
/// at all. That difference matters: "never" hides bugs, "1e-40" does not.
pub fn atmospheric_density(altitude_m: f64) -> f64 {
    let h_km = altitude_m / 1000.0;
    if h_km <= 0.0 {
        return DENSITY_LAYERS[0].1;
    }
    let mut layer = DENSITY_LAYERS[0];
    for l in DENSITY_LAYERS {
        if h_km >= l.0 {
            layer = *l;
        }
    }
    let (base_km, base_rho, scale_km) = layer;
    base_rho * (-(h_km - base_km) / scale_km).exp()
}

/// How a spacecraft's shape and mass resist drag: `Cd * A / m`, in m^2/kg.
///
/// This one number decides everything about decay. Bigger is draggier. The ISS
/// is roughly 0.01 (a huge solar array area against 420 tonnes). A dense
/// mothership is far lower, because mass grows with the cube of size and area
/// only with the square, so a big ship is intrinsically harder to drag down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BallisticCoefficient(pub f64);

impl BallisticCoefficient {
    /// ISS-like: large arrays, 420 tonnes. `Cd 2.2 * ~2000 m^2 / 420_000 kg`.
    pub const ISS_LIKE: Self = Self(0.0105);
    /// A dense mothership: the square-cube law working in your favour.
    pub const MOTHERSHIP: Self = Self(0.0005);
}

/// Rate of change of semi-major axis from drag, m/s (negative: orbits shrink).
///
/// For a near-circular orbit the standard result is
/// `da/dt = -(Cd A / m) * rho * sqrt(mu * a)`, which comes from equating the
/// drag power `F * v` to the rate of change of orbital energy `-mu m / 2a`.
pub fn decay_rate_m_per_s(semi_major_axis_m: f64, b: BallisticCoefficient, mu: f64) -> f64 {
    let altitude = semi_major_axis_m - R_EARTH;
    if altitude <= 0.0 {
        return 0.0;
    }
    let rho = atmospheric_density(altitude);
    -b.0 * rho * (mu * semi_major_axis_m).sqrt()
}

/// Seconds from this orbit to reentry with the engines cut, or `None` if the
/// orbit is stable on any timescale worth reporting.
///
/// Integrated with a fixed step rather than solved in closed form, because
/// density depends on altitude and altitude is what we are solving for. The
/// step is deliberately fixed (not frame-derived) so the answer is identical on
/// every machine, which matters the moment two players share a world.
///
/// `cap_seconds` bounds the search; past it the orbit is "effectively stable"
/// and the caller should say so rather than print a number nobody will live to
/// see.
pub fn time_to_reentry_s(
    semi_major_axis_m: f64,
    b: BallisticCoefficient,
    mu: f64,
    cap_seconds: f64,
) -> Option<f64> {
    const STEP_S: f64 = 60.0;
    let mut a = semi_major_axis_m;
    let mut t = 0.0;
    while a - R_EARTH > REENTRY_ALTITUDE_M {
        if t >= cap_seconds {
            return None;
        }
        let da = decay_rate_m_per_s(a, b, mu) * STEP_S;
        // A step that changes nothing means drag is immeasurable here; bail
        // rather than spin to the cap one wasted minute at a time.
        if da.abs() < 1e-9 {
            return None;
        }
        a += da;
        t += STEP_S;
    }
    Some(t)
}

/// The e-folding time of the unstable mode at a collinear Lagrange point (L1 or
/// L2), in the same time units as the system's orbital period.
///
/// Collinear points are saddle points: an offset grows exponentially, so
/// station-keeping is not optional there. This returns the time for an offset to
/// grow by a factor of e. Multiply by `ln(10)` for a tenfold drift.
///
/// Sanity check against reality: for Sun-Earth this yields about 23 days, which
/// is why JWST and SOHO perform a station-keeping burn roughly every three
/// weeks.
pub fn collinear_efold_time(mass_ratio: f64, orbital_period: f64) -> f64 {
    let gamma = l1_gamma(mass_ratio);
    // Effective gravity gradient at the point, in normalized units.
    let c = (1.0 - mass_ratio) / (1.0 - gamma).powi(3) + mass_ratio / gamma.powi(3);
    // Unstable root of the linearised planar characteristic equation
    // lambda^4 + (2 - c) lambda^2 + (1 + c - 2 c^2) = 0.
    let lambda = (((c - 2.0) + (9.0 * c * c - 8.0 * c).sqrt()) / 2.0).sqrt();
    orbital_period / (std::f64::consts::TAU * lambda)
}

/// Distance from the smaller primary to L1, as a fraction of the separation.
/// Solves the quintic by bisection: robust, and this is not a hot path.
fn l1_gamma(mass_ratio: f64) -> f64 {
    let f = |g: f64| {
        g.powi(5) - (3.0 - mass_ratio) * g.powi(4) + (3.0 - 2.0 * mass_ratio) * g.powi(3)
            - mass_ratio * g * g
            + 2.0 * mass_ratio * g
            - mass_ratio
    };
    let (mut lo, mut hi) = (1e-10_f64, 0.9_f64);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(lo) * f(mid) <= 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Are the triangular points (L4/L5) stable for this mass ratio?
///
/// They are, below the Routh value 0.03852. Earth-Moon is 0.0121 and Sun-Earth
/// is 3e-6, so both are comfortably stable: park there and you stay, which is
/// why Trojan asteroids exist at all.
pub fn triangular_points_stable(mass_ratio: f64) -> bool {
    mass_ratio < 0.038_520_896_5
}

/// Delta-v needed per year to cancel drag at a given orbit, m/s.
///
/// Holding an altitude means replacing exactly the energy drag removes, so this
/// is the yearly reboost budget, and it is the honest running cost of parking
/// low. Real figure for the ISS is tens of m/s per year, which this reproduces.
pub fn drag_makeup_dv_per_year(semi_major_axis_m: f64, b: BallisticCoefficient, mu: f64) -> f64 {
    const YEAR_S: f64 = 365.25 * 86_400.0;
    let da_per_s = decay_rate_m_per_s(semi_major_axis_m, b, mu);
    // For a near-circular orbit, dv = -da * sqrt(mu/a) / (2a): raising the
    // semi-major axis by da costs that much impulse.
    let dv_per_s = -da_per_s * (mu / semi_major_axis_m).sqrt() / (2.0 * semi_major_axis_m);
    dv_per_s * YEAR_S
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: f64 = 86_400.0;

    fn alt(km: f64) -> f64 {
        R_EARTH + km * 1000.0
    }

    /// Density must fall monotonically and span the many orders of magnitude
    /// that make low orbit and high orbit completely different places.
    #[test]
    fn density_falls_monotonically_across_the_whole_range() {
        let mut last = f64::INFINITY;
        for h_km in [0.0, 50.0, 100.0, 150.0, 200.0, 300.0, 400.0, 600.0, 1000.0] {
            let rho = atmospheric_density(h_km * 1000.0);
            assert!(rho < last, "density rose at {h_km} km");
            assert!(rho > 0.0, "density hit zero at {h_km} km, which hides bugs");
            last = rho;
        }
    }

    /// Pinned against the standard tabulated values. If these drift, every
    /// decay time in the game silently changes.
    #[test]
    fn density_matches_the_standard_atmosphere_at_key_altitudes() {
        let sea = atmospheric_density(0.0);
        assert!((sea - 1.225).abs() < 1e-3, "sea level was {sea}");
        // 400 km, the ISS band: a few times 1e-12.
        let iss = atmospheric_density(400_000.0);
        assert!((1e-12..1e-11).contains(&iss), "400 km density was {iss}");
        // Geostationary: essentially vacuum, but never exactly zero.
        let geo = atmospheric_density(35_786_000.0);
        assert!(geo > 0.0 && geo < 1e-20, "GEO density was {geo}");
    }

    /// Drag always removes energy: the semi-major axis can only shrink.
    #[test]
    fn drag_never_raises_an_orbit() {
        for h in [200.0, 400.0, 800.0] {
            let rate = decay_rate_m_per_s(alt(h), BallisticCoefficient::ISS_LIKE, MU_EARTH);
            assert!(rate < 0.0, "drag raised the orbit at {h} km");
        }
    }

    /// THE NUMBER THAT SIZES THE GAME EVENT. A station in the 200-300 km band
    /// with the power off has days, not months. Anything that makes this test
    /// fail has also changed how long players get to fix the reactor.
    #[test]
    fn low_orbit_decays_in_days_which_is_what_makes_the_crisis_real() {
        let b = BallisticCoefficient::ISS_LIKE;
        let cap = 200.0 * 365.25 * DAY;

        let t200 = time_to_reentry_s(alt(200.0), b, MU_EARTH, cap).expect("200 km must decay");
        assert!(
            (1.0 * DAY..4.0 * DAY).contains(&t200),
            "200 km reentry in {:.1} days; outside the 1-4 day band the crisis stops being one",
            t200 / DAY
        );

        let t300 = time_to_reentry_s(alt(300.0), b, MU_EARTH, cap).expect("300 km must decay");
        assert!(
            (20.0 * DAY..60.0 * DAY).contains(&t300),
            "300 km reentry in {:.1} days, expected roughly a month",
            t300 / DAY
        );

        assert!(t300 > t200 * 5.0, "a 100 km lift must buy a lot more than a little time");
    }

    /// Geostationary cannot decay from drag. The operator's lose-power-and-crash
    /// event is a LOW-ORBIT story, and this is the test that says so.
    #[test]
    fn geostationary_does_not_decay_and_therefore_cannot_crash() {
        let t = time_to_reentry_s(
            alt(35_786.0),
            BallisticCoefficient::ISS_LIKE,
            MU_EARTH,
            1000.0 * 365.25 * DAY,
        );
        assert!(t.is_none(), "GEO must not decay from drag; got {t:?}");
    }

    /// The square-cube law is on a big ship's side: same orbit, far slower
    /// decay. This is why a mothership must park LOW to be in danger at all.
    #[test]
    fn a_dense_mothership_decays_far_slower_than_a_station() {
        let cap = 500.0 * 365.25 * DAY;
        let station = time_to_reentry_s(alt(250.0), BallisticCoefficient::ISS_LIKE, MU_EARTH, cap)
            .expect("station decays");
        let ship = time_to_reentry_s(alt(250.0), BallisticCoefficient::MOTHERSHIP, MU_EARTH, cap)
            .expect("ship decays eventually");
        assert!(
            ship > station * 10.0,
            "ship {:.0} d vs station {:.0} d: the square-cube advantage vanished",
            ship / DAY,
            station / DAY
        );
    }

    /// Checked against reality: JWST and SOHO really do correct roughly every
    /// three weeks. If this stops matching, the model has drifted from physics.
    #[test]
    fn sun_earth_lagrange_matches_the_real_three_week_cadence() {
        let tau = collinear_efold_time(3.0034e-6, 365.256);
        assert!(
            (20.0..26.0).contains(&tau),
            "Sun-Earth L1 e-folding was {tau:.1} days; the real figure is about 23"
        );
    }

    /// Earth-Moon collinear points are far twitchier, which makes them a
    /// genuinely different parking decision rather than a reskin.
    #[test]
    fn earth_moon_lagrange_is_a_much_shorter_leash() {
        let tau = collinear_efold_time(0.01215, 27.3217);
        assert!(
            (1.0..2.5).contains(&tau),
            "Earth-Moon L1 e-folding was {tau:.2} days; the real figure is about 1.5"
        );
        let sun_earth = collinear_efold_time(3.0034e-6, 365.256);
        assert!(tau < sun_earth / 5.0, "the two systems should not feel alike");
    }

    /// L4/L5 are stable in both systems we care about, which is what makes them
    /// the park-and-forget option and why Trojans exist.
    #[test]
    fn triangular_points_are_stable_for_both_systems() {
        assert!(triangular_points_stable(0.01215), "Earth-Moon L4/L5 must be stable");
        assert!(triangular_points_stable(3.0034e-6), "Sun-Earth L4/L5 must be stable");
        // And the Routh limit must actually bite somewhere, or the check is
        // decorative: a mass ratio above 0.0385 is genuinely unstable.
        assert!(!triangular_points_stable(0.05));
    }

    /// The running cost of parking low, checked against the real ISS budget of
    /// roughly tens of m/s per year.
    #[test]
    fn iss_altitude_reboost_budget_is_realistic() {
        let dv = drag_makeup_dv_per_year(alt(400.0), BallisticCoefficient::ISS_LIKE, MU_EARTH);
        assert!(
            (5.0..200.0).contains(&dv),
            "400 km reboost budget was {dv:.1} m/s per year; the real figure is tens"
        );
        // And parking lower must cost dramatically more, or altitude is not a
        // real decision.
        let low = drag_makeup_dv_per_year(alt(250.0), BallisticCoefficient::ISS_LIKE, MU_EARTH);
        assert!(low > dv * 5.0, "250 km ({low:.0}) should cost far more than 400 km ({dv:.0})");
    }
}

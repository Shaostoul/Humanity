//! Where the stars are, and how much of them a person can see (2026-10-04).
//!
//! Two pure functions the star pass needs every frame, kept apart from the
//! GPU code in `stars.rs` so the arithmetic is tested on its own:
//!
//! 1. [`equatorial_to_world`]: the turn that puts the star catalogue's sky
//!    (J2000 equatorial, x = the vernal equinox, z = the north celestial pole)
//!    into this engine's world frame (+Y = Earth's spin axis, north).
//! 2. [`twilight_fades`]: how much of the Milky Way, the star field and the
//!    brightest stars survive the sky brightening at dawn and dusk.
//!
//! WHY THE SKY NEEDED A TURN (BUG-139). The star pass drew the catalogue in
//! its raw axes, so the north celestial pole sat at world +Z while Earth spins
//! about world +Y. Polaris was on the sky's equator, rising and setting, and
//! from Silverdale before dawn the south-east showed the southern Milky Way
//! around Crux and Carina, which never rises at 47.6 degrees north. Every
//! night sky in the game was the wrong one.
//!
//! WHY THE TURN DEPENDS ON THE DATE. The sky has to agree with the sun, which
//! is what sets the game's clock: a planet's spin is defined from the sun's
//! direction (`dev_travel::planet_spin_from_time`), so local noon is wherever
//! the sun is. The sun's place among the stars, its right ascension, moves
//! about a degree a day through the year. So the turn about the pole is chosen
//! each frame to set the world sun on the real sun's right ascension for
//! today, and the sidereal time at any place and hour follows from that: the
//! meridian at local apparent solar time T carries right ascension
//! `ra_sun + (T - 12 h)`. That is the definition of apparent solar time, so the
//! stars stand where they really stand at that moment, to the accuracy of the
//! solar formula below (about 0.01 degree).
//!
//! What the model still cannot do: the game has no axial tilt, so the sun
//! itself stays on the sky's equator (declination 0). On 2026-10-04 the real
//! sun is at -4.5 degrees, so the game's sun rises a few degrees north of
//! where it really does; at the equinoxes the error is nil and at the
//! solstices it is 23.4 degrees. The planets and the Moon come from the
//! orbit model in `cosmos.rs`, which maps the ecliptic into the world with a
//! reflection (BUG-140), so their places among these stars are not right
//! either. The stars, the Milky Way and the constellations are.

use glam::{DQuat, DVec3};

/// The sun's apparent right ascension, radians, for a moment given as days
/// since J2000.0 (2000-01-01 12:00, the engine's `sim_t / 86400`).
///
/// The Astronomical Almanac's low-precision solar coordinates (good to about
/// 0.01 degree between 1950 and 2050), the standard formula for this job:
/// mean longitude L and mean anomaly g, ecliptic longitude
/// `lambda = L + 1.915 sin g + 0.020 sin 2g`, obliquity
/// `eps = 23.439 - 0.0000004 n`, and `ra = atan2(cos eps sin lambda, cos lambda)`.
pub fn sun_right_ascension(days_since_j2000: f64) -> f64 {
    let n = days_since_j2000;
    let l = (280.460 + 0.985_647_4 * n).to_radians();
    let g = (357.528 + 0.985_600_3 * n).to_radians();
    let lambda = l + (1.915 * g.sin() + 0.020 * (2.0 * g).sin()).to_radians();
    let eps = (23.439 - 0.000_000_4 * n).to_radians();
    (eps.cos() * lambda.sin()).atan2(lambda.cos())
}

/// The turn from the star catalogue's equatorial axes into the world frame.
///
/// `sun_world` is the sun's direction from Earth in the world frame
/// (`state.sun_world_pos`); only its azimuth about +Y is used, measured the
/// way the spin measures it, `atan2(-z, x)` (`frame_lock::sun_azimuth`).
///
/// Built in two steps. First a fixed quarter turn about X takes the
/// catalogue's pole (+z) to the world's +Y, its x axis to +X, and its y axis
/// to -Z, so a star at right ascension `ra` on the celestial equator lands at
/// world azimuth `ra` in that same `atan2(-z, x)` measure. Then a turn about
/// +Y by `sun_azimuth - ra_sun` moves the real sun's right ascension onto the
/// world sun. Both are proper rotations: the sky is never mirrored.
pub fn equatorial_to_world(sun_world: DVec3, days_since_j2000: f64) -> DQuat {
    let sun_azimuth = (-sun_world.z).atan2(sun_world.x);
    let ra_sun = sun_right_ascension(days_since_j2000);
    DQuat::from_rotation_y(sun_azimuth - ra_sun) * DQuat::from_rotation_x(-std::f64::consts::FRAC_PI_2)
}

/// How much of each sky layer is drawn, 0..1: the star camera's fade slot
/// (see `StarRenderer::update_camera`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkyFades {
    /// The Milky Way glow.
    pub glow: f32,
    /// Every star point, and the constellation figures drawn with them.
    pub points: f32,
    /// The halos on the brightest stars (magnitude 2 and brighter).
    pub halos: f32,
}

impl SkyFades {
    /// Nothing faded: night, or space.
    pub const FULL: SkyFades = SkyFades { glow: 1.0, points: 1.0, halos: 1.0 };

    /// The four floats the shaders read (`camera.sun_color` in the star
    /// camera's uniform): x = points, y = halos, z = unused, w = glow.
    pub fn as_uniform(self) -> [f32; 4] {
        [self.points, self.halos, 0.0, self.glow]
    }
}

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// What a person under the sky can still see of it with the sun at
/// `sun_elevation_deg` (negative below the horizon), standing `altitude_m`
/// over the ground.
///
/// The renderer has no eye adaptation, so without this the star layers kept
/// their full night brightness under a dawn sky: the Milky Way stood in a
/// gold sky with the sun's disc already up (the 2026-10-04 review of the
/// Silverdale clip). What really hides the stars is the sky getting brighter
/// than they are, which follows the sun's depression below the horizon, in
/// this order: the Milky Way first (it needs a fully dark sky), then the star
/// field (the points are drawn at one brightness for every naked-eye star, so
/// they fade as a layer), and the brightest stars, the halo set (magnitude 2
/// and brighter), last.
///
/// WHERE THE RAMPS SIT, AND WHY NOT AT THE NAKED-EYE LIMITS. For a dark-adapted
/// eye the Milky Way is gone with the sun 12 degrees down, the field is down
/// to first-magnitude stars by 7 down, and the brightest go at sunrise
/// ([`NAKED_EYE`]). But the sky this engine DRAWS stays black until the sun
/// is about 5 degrees down (BUG-141: single scattering and no eye
/// adaptation; measured on the rig over Silverdale, 2026-10-04: black at
/// -10 and -7.6, the first orange band at -5, a lit dawn by -2.5), so at
/// those limits the frame went empty for half an hour of game time: no
/// stars, and no twilight in their place. So each layer fades
/// as the DRAWN sky brightens over it ([`DRAWN_SKY`]): the stars never stand
/// in a lit sky, and the sky is never an empty black. When BUG-141 gives the
/// sky its real twilight, switch to [`NAKED_EYE`].
///
/// Above the air there is no sky to hide them: the fades ease back to full
/// between 60 and 120 km, so nothing pops at the daylight gate's 120 km line.
pub fn twilight_fades(sun_elevation_deg: f64, altitude_m: f64) -> SkyFades {
    fades_with(&DRAWN_SKY, sun_elevation_deg, altitude_m)
}

/// Where each layer is fully seen and where it is gone, as sun elevations in
/// degrees: `[full, gone]` for the glow, the points and the halos.
pub struct TwilightRamps {
    pub glow: [f64; 2],
    pub points: [f64; 2],
    pub halos: [f64; 2],
}

/// The ramps matched to the engine's drawn sky (see [`twilight_fades`]):
/// the glow is gone as the first colour reaches the drawn sky (about 5
/// degrees down), the field fades through the drawn sky's dawn glow, and
/// the brightest stars are gone just after the sun's disc clears the
/// horizon. A first set, with the Milky Way gone at 8 degrees down, left the
/// drawn sky black and nearly starless for two and a half seconds of the
/// Silverdale clip.
pub const DRAWN_SKY: TwilightRamps = TwilightRamps { glow: [-11.0, -5.0], points: [-8.0, -1.0], halos: [-5.0, 1.5] };

/// The naked-eye limits for a dark-adapted eye, for when the drawn sky has
/// its real twilight (BUG-141).
pub const NAKED_EYE: TwilightRamps = TwilightRamps { glow: [-18.0, -12.0], points: [-18.0, -7.0], halos: [-10.0, 0.0] };

/// [`twilight_fades`] on a given set of ramps.
pub fn fades_with(ramps: &TwilightRamps, sun_elevation_deg: f64, altitude_m: f64) -> SkyFades {
    let up_in_the_air = 1.0 - smoothstep(60_000.0, 120_000.0, altitude_m);
    let h = sun_elevation_deg;
    let fade = |[full_at, gone_at]: [f64; 2]| {
        let seen = 1.0 - smoothstep(full_at, gone_at, h);
        (1.0 - up_in_the_air * (1.0 - seen)) as f32
    };
    SkyFades { glow: fade(ramps.glow), points: fade(ramps.points), halos: fade(ramps.halos) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SILVERDALE: (f64, f64) = (47.645, -122.695);

    /// Days since J2000.0 of a UTC instant (the engine's own time base:
    /// `unix - 946_728_000`, over a day).
    fn days(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> f64 {
        // Days from 1970-01-01 to the civil date (Howard Hinnant's algorithm).
        let (y, mo) = if mo <= 2 { (y - 1, mo + 9) } else { (y, mo - 3) };
        let era = y.div_euclid(400);
        let yoe = (y - era * 400) as i64;
        let doy = (153 * mo as i64 + 2) / 5 + d as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let unix_days = era as i64 * 146_097 + doe - 719_468;
        let unix = unix_days as f64 * 86_400.0 + (h * 3600 + mi * 60) as f64;
        (unix - 946_728_000.0) / 86_400.0
    }

    fn equatorial(ra_deg: f64, dec_deg: f64) -> DVec3 {
        let (ra, dec) = (ra_deg.to_radians(), dec_deg.to_radians());
        DVec3::new(dec.cos() * ra.cos(), dec.cos() * ra.sin(), dec.sin())
    }

    /// The sun's world direction from the orbit model: ecliptic longitude
    /// `lambda` lands at world (cos, 0, sin), the mapping `cosmos.rs` uses.
    fn model_sun(n: f64) -> DVec3 {
        let l = (280.460 + 0.985_647_4 * n).to_radians();
        let g = (357.528 + 0.985_600_3 * n).to_radians();
        let lambda = l + (1.915 * g.sin() + 0.020 * (2.0 * g).sin()).to_radians();
        DVec3::new(lambda.cos(), 0.0, lambda.sin())
    }

    /// Altitude and compass azimuth (degrees) of a catalogue direction for an
    /// observer the game places at `lat, lon` with the game clock at `hour`
    /// (the global, longitude-0 hour), through the game's own placement: the
    /// spin from `planet_spin_from_time`, the ground from `latlon_to_dir_f64`
    /// turned by that spin, east and north from that up.
    fn game_alt_az(star: DVec3, n: f64, lat: f64, lon: f64, hour: f64) -> (f64, f64) {
        let sun = model_sun(n);
        let spin = crate::dev_travel::planet_spin_from_time(hour, (-sun.z).atan2(sun.x));
        let turn = DQuat::from_rotation_y(spin);
        let up = turn * crate::terrain::osm_region::latlon_to_dir_f64(lat, lon);
        let east = DVec3::Y.cross(up).normalize();
        let north = up.cross(east);
        let w = equatorial_to_world(sun, n) * star;
        (w.dot(up).asin().to_degrees(), w.dot(east).atan2(w.dot(north)).to_degrees().rem_euclid(360.0))
    }

    /// The real sky, from the IAU sidereal-time formula: altitude and azimuth
    /// of a star at a UTC moment (days since J2000) from `lat, lon`.
    fn real_alt_az(ra_deg: f64, dec_deg: f64, n: f64, lat: f64, lon: f64) -> (f64, f64) {
        let gmst = (280.460_618_37 + 360.985_647_366_29 * n).rem_euclid(360.0);
        let ha = (gmst + lon - ra_deg).to_radians();
        let (d, la) = (dec_deg.to_radians(), lat.to_radians());
        let alt = (la.sin() * d.sin() + la.cos() * d.cos() * ha.cos()).asin();
        let az = (-d.cos() * ha.sin()).atan2(la.cos() * d.sin() - la.sin() * d.cos() * ha.cos());
        (alt.to_degrees(), az.to_degrees().rem_euclid(360.0))
    }

    /// The game clock that puts the sun where it really is at that moment:
    /// the sun's real hour angle at longitude 0, read as a 24-hour dial
    /// (the clock is longitude-0 apparent solar time by construction).
    fn game_hour_for(n: f64) -> f64 {
        let gmst = (280.460_618_37 + 360.985_647_366_29 * n).rem_euclid(360.0);
        let ha = (gmst - sun_right_ascension(n).to_degrees() + 540.0).rem_euclid(360.0) - 180.0;
        12.0 + ha / 15.0
    }

    fn close_angle(a: f64, b: f64) -> f64 {
        ((a - b + 540.0).rem_euclid(360.0) - 180.0).abs()
    }

    /// The solar formula against the equinoxes, where the sun's right
    /// ascension is exactly 0 and 12 h: 2026-03-20 14:46 UTC and 2026-09-23
    /// 00:05 UTC.
    #[test]
    fn the_sun_crosses_the_equinoxes_on_time() {
        let march = sun_right_ascension(days(2026, 3, 20, 14, 46)).to_degrees();
        let september = sun_right_ascension(days(2026, 9, 23, 0, 5)).to_degrees();
        assert!(close_angle(march, 0.0) < 0.1, "March equinox RA {march}");
        assert!(close_angle(september, 180.0) < 0.1, "September equinox RA {september}");
    }

    /// THE WRONG SKY (BUG-139). Polaris is 0.74 degrees from the north
    /// celestial pole, so from Silverdale it stands due north at the height
    /// of the latitude, 47.6 degrees, at every hour of every night.
    ///
    /// Red check, run 2026-10-04 with `equatorial_to_world` returning the
    /// identity (the raw catalogue axes the star pass drew in): it failed at
    /// the first hour, "Polaris at hour 0: altitude 29.3, azimuth 127.1"
    /// (the pole sat on the sky's equator and swung round with the day).
    #[test]
    fn polaris_stands_due_north_at_the_latitude_all_night() {
        let polaris = equatorial(37.954_6, 89.264_1);
        let (lat, lon) = SILVERDALE;
        for n in [days(2026, 10, 4, 0, 0), days(2027, 1, 15, 0, 0), days(2027, 6, 21, 0, 0)] {
            for hour in [0.0, 3.0, 6.0, 9.0, 12.0, 15.0, 18.0, 21.0] {
                let (alt, az) = game_alt_az(polaris, n, lat, lon, hour);
                assert!(
                    (alt - lat).abs() < 0.8 && close_angle(az, 0.0) < 1.2,
                    "Polaris at hour {hour}: altitude {alt:.1}, azimuth {az:.1}"
                );
            }
        }
    }

    /// The sky the landing clip films, against the real one: Silverdale,
    /// 2026-10-04, 12:52 UTC (05:52 local, about an hour before dawn). Each
    /// star's altitude and azimuth through the game's placement match the
    /// IAU sidereal-time sky within 0.1 degree, and the southern stars that
    /// the raw axes put over the horizon are below it.
    ///
    /// Red check, run 2026-10-04 with the identity turn: "Sirius: game
    /// (alt 53.6, az 30.5), real (alt 23.7, az 161.4)".
    #[test]
    fn the_dawn_sky_over_silverdale_is_the_real_one() {
        let n = days(2026, 10, 4, 12, 52);
        let (lat, lon) = SILVERDALE;
        let hour = game_hour_for(n);
        for (name, ra, dec) in [
            ("Sirius", 101.287, -16.716),
            ("Procyon", 114.825, 5.225),
            ("Betelgeuse", 88.793, 7.407),
            ("Rigel", 78.634, -8.202),
            ("Capella", 79.172, 45.998),
            ("Regulus", 152.093, 11.967),
            ("Deneb", 310.358, 45.280),
        ] {
            let game = game_alt_az(equatorial(ra, dec), n, lat, lon, hour);
            let real = real_alt_az(ra, dec, n, lat, lon);
            assert!(
                (game.0 - real.0).abs() < 0.1 && close_angle(game.1, real.1) < 0.1,
                "{name}: game (alt {:.1}, az {:.1}), real (alt {:.1}, az {:.1})",
                game.0,
                game.1,
                real.0,
                real.1
            );
        }
        // Never up from 47.6 N (nothing south of -42.4 rises there): Acrux,
        // in the Southern Cross, and Canopus are below the horizon.
        for (name, ra, dec) in [("Acrux", 186.650, -63.099), ("Canopus", 95.988, -52.696)] {
            let (alt, _) = game_alt_az(equatorial(ra, dec), n, lat, lon, hour);
            assert!(alt < 0.0, "{name} is over the horizon at Silverdale: {alt:.1}");
        }
    }

    /// The turn is a rotation (never a mirror), and the sun lands on its own
    /// right ascension, on the sky's equator.
    ///
    /// Red check, run 2026-10-04 with the identity turn: "the pole is not
    /// world +Y: DVec3(0.0, 0.0, 1.0)".
    #[test]
    fn the_turn_is_proper_and_carries_the_sun() {
        let n = days(2026, 10, 4, 12, 0);
        let sun = model_sun(n);
        let q = equatorial_to_world(sun, n);
        let (x, y, z) = (q * DVec3::X, q * DVec3::Y, q * DVec3::Z);
        assert!((x.cross(y) - z).length() < 1e-12, "the sky is mirrored");
        assert!((z - DVec3::Y).length() < 1e-12, "the pole is not world +Y: {z:?}");
        let ra = sun_right_ascension(n);
        let on_sky = q * DVec3::new(ra.cos(), ra.sin(), 0.0);
        assert!((on_sky - sun.normalize()).length() < 1e-9, "the sun's RA is not on the world sun");
    }

    /// The fades the game draws with: everything at night, nothing in a lit
    /// sky, and through the drawn sky's dark stretch (BUG-141: black until
    /// the sun is about 5 degrees down) the Milky Way and the stars stay, so
    /// the frame is never an empty black.
    ///
    /// Red check, run 2026-10-04 with every fade held at 1 (no twilight, the
    /// star pass as it was): "the Milky Way is gone once the drawn sky lights"
    /// failed (left 1.0, right 0.0).
    #[test]
    fn the_sky_fades_with_the_drawn_dawn() {
        let ground = 300.0;
        assert_eq!(twilight_fades(-25.0, ground), SkyFades::FULL, "night");
        assert_eq!(twilight_fades(-15.0, ground), SkyFades::FULL, "still night in the drawn sky");
        let dark = twilight_fades(-10.0, ground);
        assert!(
            dark.glow > 0.8 && dark.points == 1.0 && dark.halos == 1.0,
            "the drawn sky is black at -10: keep the sky's night: {dark:?}"
        );
        assert_eq!(twilight_fades(-5.0, ground).glow, 0.0, "the Milky Way is gone once the drawn sky lights");
        let civil = twilight_fades(-1.0, ground);
        assert_eq!(civil.points, 0.0, "the star field is gone in the dawn glow");
        assert!(civil.halos > 0.1, "the brightest stars outlast it: {civil:?}");
        assert_eq!(twilight_fades(1.5, ground).halos, 0.0, "nothing left once the sun is up");
    }

    /// Both ramp sets fade in the right order and only one way: the Milky
    /// Way first, the brightest stars last, and nothing comes back as the
    /// sun climbs. The naked-eye set (for when BUG-141 is fixed) is pinned
    /// to its limits so a switch to it is a switch to the real thing.
    ///
    /// Red check, run 2026-10-04 with `fades_with` ignoring its ramps (every
    /// fade 1): "naked eye: no Milky Way 12 degrees down" failed (left 1.0,
    /// right 0.0).
    #[test]
    fn the_layers_fade_in_order_and_one_way() {
        for ramps in [&DRAWN_SKY, &NAKED_EYE] {
            assert!(ramps.glow[1] <= ramps.points[1] && ramps.points[1] <= ramps.halos[1], "fade order");
            let mut last = SkyFades::FULL;
            for tenth in -250..60 {
                let f = fades_with(ramps, tenth as f64 / 10.0, 300.0);
                assert!(f.glow <= last.glow && f.points <= last.points && f.halos <= last.halos);
                last = f;
            }
        }
        assert_eq!(fades_with(&NAKED_EYE, -12.0, 300.0).glow, 0.0, "naked eye: no Milky Way 12 degrees down");
        assert_eq!(fades_with(&NAKED_EYE, -7.0, 300.0).points, 0.0, "naked eye: the field gone by 7 down");
        assert_eq!(fades_with(&NAKED_EYE, 0.0, 300.0).halos, 0.0, "naked eye: nothing at sunrise");
    }

    /// Space has no sky to hide the stars, and the climb out of the air
    /// brings them back smoothly.
    ///
    /// Red check, run 2026-10-04 with every fade held at 1: "halfway out of
    /// the air: SkyFades { glow: 1.0, points: 1.0, halos: 1.0 }" (the ramp
    /// assertion; with no fade at all there is nothing to bring back).
    #[test]
    fn above_the_air_nothing_fades() {
        assert_eq!(twilight_fades(30.0, 150_000.0), SkyFades::FULL);
        assert_eq!(twilight_fades(30.0, 120_000.0), SkyFades::FULL);
        let mid = twilight_fades(30.0, 90_000.0);
        assert!(mid.points > 0.4 && mid.points < 0.6, "halfway out of the air: {mid:?}");
    }
}

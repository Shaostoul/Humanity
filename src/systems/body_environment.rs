//! Per-body environment: which world the player is on, and what its air,
//! water, and temperature are like at the player's position.
//!
//! This is increment 4 of the artificial-planet ladder
//! (docs/design/artificial-planet.md): before it, the environment was
//! Earth-global. WeatherSystem could rain on the Moon, Mars breathed like
//! Earth (the alien-world hook `AtmosphereSystem::set_outside_atmosphere`
//! had no production caller), and one hardcoded Earth season table fed
//! temperature to vitals, farming, and hydrology on every body.
//!
//! The flow: once per frame the native main loop (lib.rs, right before the
//! survival environment block) publishes a `BodyEnvironment` snapshot of
//! the frame-locked body into the DataStore under key `"body_environment"`
//! (a plain value, the `environment_context` pattern: systems only read it).
//! WeatherSystem gates precipitation and retunes its temperature baseline
//! from it; AtmosphereSystem sets the outside air from it; the survival
//! context reads outside-the-hull breathability from it.
//!
//! Everything in this file is PURE data + math (no ECS, no engine state),
//! so it unit-tests directly and compiles in every feature set (the relay
//! build includes systems/).

use serde::{Deserialize, Serialize};

use crate::systems::atmosphere::Atmosphere;
use crate::systems::time::Season;
use crate::systems::weather::WeatherCondition;
use crate::terrain::planet::PlanetDef;

/// The latitude (degrees) where Earth's calibrated seasonal table applies
/// exactly. The old global table was written for a temperate mid-latitude
/// homestead, so 45 degrees is the calibration point: at 45 deg / sea
/// level the new model reproduces the old values bit-for-bit (zero
/// gameplay regression on Earth), and every modulation term is zero there.
pub const REFERENCE_LATITUDE_DEG: f32 = 45.0;

/// Pole-to-equator temperature span (Celsius) of the cosine insolation
/// gradient. Fit to real Earth annual means: equator ~+27 C, poles ~-30 C,
/// mid-latitudes ~+10 C; `mean + 57 * (cos(lat) - cos(45))` hits all three
/// within a couple of degrees.
pub const LATITUDE_GRADIENT_C: f32 = 57.0;

/// Tropospheric lapse rate: air cools ~6.5 K per km of altitude. Applied
/// only on bodies that actually have an atmosphere (no air = no lapse).
pub const LAPSE_RATE_C_PER_KM: f32 = 6.5;

/// Altitude (km) where the lapse stops in this v1 model (Earth's
/// tropopause). Above it temperature holds instead of dropping forever.
pub const TROPOPAUSE_ALTITUDE_KM: f32 = 11.0;

/// Day/night swing amplitude (Celsius, half the full span) on AIRLESS
/// bodies. With no atmosphere to store and transport heat, the sunlit and
/// dark sides differ enormously (the real Moon spans roughly 250 K).
pub const AIRLESS_DIURNAL_SWING_C: f32 = 120.0;

/// Day/night swing amplitude on non-Earth bodies that DO have an
/// atmosphere (thin air moderates the swing but does not erase it; Mars
/// nights are far colder than its days). Earth gets zero here because its
/// calibrated seasonal table already stands in for its daily variation.
pub const THIN_AIR_DIURNAL_SWING_C: f32 = 30.0;

/// Local hour of the warmest moment of the day (mid-afternoon), the peak
/// of the diurnal cosine. The coldest moment lands 12 hours opposite.
pub const DIURNAL_PEAK_HOUR: f32 = 14.0;

/// Ceiling (meters above the surface) for unsuited breathing on a
/// breathable body. Earth's death zone starts near 8 km; above it the
/// vacuum/suit rules apply even though the body itself is breathable.
pub const BREATHABLE_CEILING_M: f32 = 8_000.0;

/// Snapshot of the frame-locked body's environment at the player's
/// position. Published each frame by the native main loop; consumed by
/// WeatherSystem, AtmosphereSystem, and the survival environment context.
///
/// The DEFAULT is the "home frame": the station canonically rides high
/// Earth orbit, so with no body frame-locked the weather sim keeps
/// simulating Earth (home farming and the garden's seasons behave exactly
/// as they always have) while `locked: false` keeps the outside of the
/// hull a vacuum (space). This is precisely the pre-increment behavior,
/// which is why absent data (unit tests, headless relay) changes nothing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyEnvironment {
    /// Cosmos body id ("earth", "moon", "mars", ...). Also the key into
    /// data/planets/<id>.ron.
    pub body_id: String,
    /// True when a body is actually frame-locked (the player is at a
    /// world, not floating in open space in the home frame).
    pub locked: bool,
    /// PlanetDef.atmosphere_color.is_some(): the body has air at all.
    pub has_atmosphere: bool,
    /// PlanetDef.has_water: liquid surface water exists (rain/snow/fog
    /// need somewhere to evaporate from).
    pub has_water: bool,
    /// PlanetDef.breathable: an unsuited human can breathe the open air.
    /// Only Earth ships true today.
    pub breathable: bool,
    /// Mean surface temperature in Kelvin, from the cosmos catalog
    /// (data/star_systems/sol.json `mean_temperature_k`).
    pub mean_temperature_k: f32,
    /// Player latitude on the body, degrees (+north). Spin-invariant
    /// because bodies spin about +Y.
    pub latitude_deg: f32,
    /// Player altitude above the body's NOMINAL surface radius
    /// (anchor.length() - def.radius), meters. The frame-lock anchor
    /// sits on the drawn ground, so this INCLUDES terrain elevation: a
    /// 4 km plateau reads ~4000 and the mountain lapse engages. It is
    /// deliberately NOT the drawn-ground readout
    /// (gui_state.surface_altitude_m), which is height above the ground
    /// under your feet and therefore 0 whenever you stand anywhere,
    /// which silenced the lapse entirely (review fix). May be slightly
    /// negative below the nominal sphere (sea floor, deep valleys);
    /// consumers clamp where needed.
    pub altitude_m: f32,
}

impl Default for BodyEnvironment {
    fn default() -> Self {
        Self {
            body_id: "earth".to_string(),
            locked: false,
            has_atmosphere: true,
            has_water: true,
            breathable: true,
            mean_temperature_k: 288.0,
            latitude_deg: REFERENCE_LATITUDE_DEG,
            altitude_m: 0.0,
        }
    }
}

impl BodyEnvironment {
    /// Convenience constructor for an airless world (Moon, Pluto).
    pub fn airless(body_id: &str, mean_temperature_k: f32) -> Self {
        Self {
            body_id: body_id.to_string(),
            locked: true,
            has_atmosphere: false,
            has_water: false,
            breathable: false,
            mean_temperature_k,
            latitude_deg: 0.0,
            altitude_m: 0.0,
        }
    }

    /// Convenience constructor for a world with air but no surface water
    /// and unbreathable air (Mars).
    pub fn dry_atmosphere(body_id: &str, mean_temperature_k: f32) -> Self {
        Self {
            body_id: body_id.to_string(),
            locked: true,
            has_atmosphere: true,
            has_water: false,
            breathable: false,
            mean_temperature_k,
            latitude_deg: 0.0,
            altitude_m: 0.0,
        }
    }

    /// Can an unsuited human breathe OUTSIDE the hull right here?
    /// Requires actually standing at a body (locked), that body having
    /// breathable air, and being below the breathable ceiling. Open space
    /// (the home-orbit default has `locked: false`) is always a no, which
    /// preserves the vacuum drain when floating outside the station.
    pub fn breathable_outside(&self) -> bool {
        self.locked
            && self.has_atmosphere
            && self.breathable
            && self.altitude_m < BREATHABLE_CEILING_M
    }
}

/// Earth's calibrated season -> Celsius table, verbatim from the original
/// WeatherSystem (v0.x global table). It is the profile the whole
/// survival/farming balance was tuned against, so it stays THE Earth
/// baseline; latitude/altitude/day-night now modulate around it instead
/// of replacing it.
pub fn earth_season_c(season: Season) -> f32 {
    match season {
        Season::Spring => 15.0,
        Season::Summer => 30.0,
        Season::Autumn => 12.0,
        Season::Winter => -2.0,
    }
}

/// Earth's annual mean of the calibrated table (13.75 C). Used to split
/// the table into mean + seasonal deviation so the seasonal amplitude can
/// scale with latitude (real equatorial seasons are nearly flat, polar
/// seasons are extreme).
const EARTH_ANNUAL_MEAN_C: f32 = (15.0 + 30.0 + 12.0 - 2.0) / 4.0;

/// The temperature baseline the weather sim ramps between conditions:
/// the body's value at the REFERENCE point (45 deg latitude, sea level,
/// diurnal mean). Earth = the calibrated seasonal table; every other
/// body = its catalog mean temperature converted to Celsius (seasons on
/// other bodies are a later increment; their axial data is not plumbed).
pub fn body_baseline_temp_c(env: &BodyEnvironment, season: Season) -> f32 {
    if env.body_id == "earth" {
        earth_season_c(season)
    } else {
        env.mean_temperature_k - 273.15
    }
}

/// Full surface temperature model v1 (Celsius) at the player's position:
///
///   Earth:  mean(latitude) + seasonal deviation * amplitude(latitude)
///           - lapse(altitude)
///   others: catalog mean + latitude gradient - lapse(altitude, if air)
///           + day/night swing (large when airless)
///
/// where mean(lat) follows the cosine insolation gradient and the
/// seasonal amplitude scales with |sin(lat)| (flat seasons at the
/// equator, extreme at the poles), both normalized so 45 deg / sea level
/// reproduces the old table exactly.
pub fn surface_temperature_c(env: &BodyEnvironment, season: Season, hour: f32) -> f32 {
    let lat_rad = env.latitude_deg.abs().min(90.0).to_radians();
    let ref_rad = REFERENCE_LATITUDE_DEG.to_radians();
    // Cosine insolation gradient, zero at the reference latitude.
    let lat_shift = LATITUDE_GRADIENT_C * (lat_rad.cos() - ref_rad.cos());

    let mut t = if env.body_id == "earth" {
        // Seasonal deviation from the annual mean, scaled by latitude:
        // amplitude 1.0 at the 45 deg reference (table preserved), ~0 at
        // the equator (perpetual near-mean), ~1.41 at the poles.
        let season_dev = earth_season_c(season) - EARTH_ANNUAL_MEAN_C;
        let amplitude = lat_rad.sin() / ref_rad.sin();
        EARTH_ANNUAL_MEAN_C + lat_shift + season_dev * amplitude
    } else {
        (env.mean_temperature_k - 273.15) + lat_shift
    };

    // Altitude lapse only exists where there is air to thin.
    if env.has_atmosphere {
        let alt_km = (env.altitude_m.max(0.0) / 1000.0).min(TROPOPAUSE_ALTITUDE_KM);
        t -= LAPSE_RATE_C_PER_KM * alt_km;
    }

    // Day/night swing: body-wide, not positional (see diurnal_swing_c).
    t + diurnal_swing_c(env, hour)
}

/// The body-wide day/night temperature swing (Celsius) at this game
/// hour. GLOBAL, not player-positional: game time is a single clock in
/// this v1 model, so the whole body warms and cools together
/// (per-longitude local solar time is a later increment). Without an
/// atmosphere the surface bakes in sunlight and radiates to
/// near-nothing in the dark (the real Moon spans roughly 250 K); thin
/// atmospheres swing moderately; Earth gets zero because its calibrated
/// seasonal table already stands in for its daily variation (the
/// zero-regression promise). Because it is global, this term rides the
/// GLOBAL weather temperature export, never the positional delta.
pub fn diurnal_swing_c(env: &BodyEnvironment, hour: f32) -> f32 {
    let swing = if !env.has_atmosphere {
        AIRLESS_DIURNAL_SWING_C
    } else if env.body_id == "earth" {
        0.0
    } else {
        THIN_AIR_DIURNAL_SWING_C
    };
    if swing <= 0.0 {
        return 0.0;
    }
    let phase = ((hour - DIURNAL_PEAK_HOUR) / 24.0) * std::f32::consts::TAU;
    swing * phase.cos()
}

/// The PLAYER-POSITIONAL part of the model ONLY: the latitude gradient
/// plus the altitude lapse, i.e. the full temperature minus everything
/// body-global (the baseline the weather sim ramps AND the day/night
/// swing). WeatherSystem adds this delta to `temperature_at_player` at
/// export time, so walking up a mountain tracks the player immediately
/// without fighting the transition machinery. The GLOBAL
/// `Weather.temperature` field that farming climate and hydrology
/// evaporation read never carries it (review split: the home station
/// sits inside Earth's frame-lock envelope at ~400 km, and letting the
/// altitude lapse into the global field would have dragged the whole
/// station's climate arctic and wrongly punished farming aboard).
/// Exactly 0.0 for the default home environment (Earth at the
/// reference latitude, sea level).
pub fn positional_temp_offset_c(env: &BodyEnvironment, season: Season, hour: f32) -> f32 {
    surface_temperature_c(env, season, hour)
        - body_baseline_temp_c(env, season)
        - diurnal_swing_c(env, hour)
}

/// Decide the BodyEnvironment to publish for the current frame lock.
/// Pure so it unit-tests directly (review fix); lib.rs only gathers the
/// inputs (frame-lock id, PlanetDef lookup, catalog temperature, anchor
/// latitude/altitude) and publishes the result.
///
/// - `body_id` None (deep space, or the home frame with the station in
///   high Earth orbit): the Earth-home DEFAULT, exactly the
///   pre-increment behavior, so home farming, the home sky, and every
///   existing test are untouched (`locked: false` keeps outside the
///   hull a vacuum).
/// - Locked to a body WITH a data/planets RON def: that def decides
///   atmosphere / water / breathability.
/// - Locked to a body WITHOUT a def (most of the ~65 catalog bodies:
///   venus, titan, io, ceres, ...): an AIRLESS, waterless, unbreathable
///   environment. Most def-less bodies really are airless rocks, and
///   the exceptions (Venus, Titan) are far better mis-modeled as
///   airless until they get def files than handed what the old code
///   fell back to: the Earth-home default, which could RAIN on Venus.
///
/// `catalog_mean_k` is the cosmos catalog's `mean_temperature_k`. The
/// catalog loader defaults an ABSENT field to 0.0 (17 bodies today),
/// and 0 K is that sentinel, not a temperature: treat <= 0.0 as unknown
/// and fall back to the same 288 K an unknown body id gets (review
/// fix).
pub fn environment_for_frame_lock(
    body_id: Option<&str>,
    def: Option<&PlanetDef>,
    catalog_mean_k: Option<f32>,
    latitude_deg: f32,
    altitude_m: f32,
) -> BodyEnvironment {
    let Some(id) = body_id else {
        return BodyEnvironment::default();
    };
    // The <= 0.0 guard is the whole FIX: without it a missing catalog
    // field reads as 0 K and drags the temperature model to absolute
    // zero instead of the honest "unknown" fallback.
    let mean_temperature_k = catalog_mean_k.filter(|k| *k > 0.0).unwrap_or(288.0);
    match def {
        Some(def) => BodyEnvironment {
            body_id: id.to_string(),
            locked: true,
            has_atmosphere: def.atmosphere_color.is_some(),
            has_water: def.has_water,
            breathable: def.breathable,
            mean_temperature_k,
            latitude_deg,
            altitude_m,
        },
        // Def-less catalog body: airless rock posture (see doc above).
        None => BodyEnvironment {
            latitude_deg,
            altitude_m,
            ..BodyEnvironment::airless(id, mean_temperature_k)
        },
    }
}

/// Clamp a rolled weather condition to what this body can physically
/// host. Airless bodies have NO weather at all (no wind, no clouds, no
/// precipitation); an atmosphere without surface water cannot rain, snow,
/// or fog, and its storms are dust storms (Mars's signature weather).
/// Water worlds pass everything through unchanged.
pub fn sanitize_condition(cond: WeatherCondition, env: &BodyEnvironment) -> WeatherCondition {
    use WeatherCondition as WC;
    if !env.has_atmosphere {
        return WC::Clear;
    }
    if !env.has_water {
        return match cond {
            WC::Rain | WC::Snow | WC::Fog => WC::Cloudy,
            WC::Storm => WC::Sandstorm,
            other => other,
        };
    }
    cond
}

/// What is outside the hull on this body, for
/// `AtmosphereSystem::set_outside_atmosphere` (the alien-world hook this
/// increment finally gives a production caller). Open space and airless
/// bodies are vacuum; a breathable body is Earth-like air; an atmosphere
/// that is not breathable is a thin unbreathable CO2 mix (Mars-like; per
/// body gas composition becomes data in a later increment).
pub fn outside_atmosphere_for(env: &BodyEnvironment) -> Atmosphere {
    if !env.locked || !env.has_atmosphere {
        return Atmosphere::vacuum();
    }
    if env.breathable {
        return Atmosphere::default();
    }
    Atmosphere::thin_unbreathable()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earth_at(lat: f32, alt: f32) -> BodyEnvironment {
        BodyEnvironment {
            locked: true,
            latitude_deg: lat,
            altitude_m: alt,
            ..Default::default()
        }
    }

    /// Zero-regression anchor: at the 45 deg reference and sea level the
    /// model reproduces the old global table exactly, for every season.
    #[test]
    fn earth_sea_level_reference_matches_the_old_table() {
        let env = earth_at(REFERENCE_LATITUDE_DEG, 0.0);
        assert_eq!(surface_temperature_c(&env, Season::Summer, 12.0), 30.0);
        assert_eq!(surface_temperature_c(&env, Season::Spring, 12.0), 15.0);
        assert_eq!(surface_temperature_c(&env, Season::Autumn, 12.0), 12.0);
        assert_eq!(surface_temperature_c(&env, Season::Winter, 12.0), -2.0);
        // And the positional offset the weather export adds is exactly 0
        // for the default home environment.
        let home = BodyEnvironment::default();
        assert_eq!(positional_temp_offset_c(&home, Season::Summer, 3.0), 0.0);
        assert_eq!(positional_temp_offset_c(&home, Season::Winter, 15.0), 0.0);
    }

    /// The equator is warmer than the pole, in summer AND winter.
    #[test]
    fn equator_warmer_than_pole() {
        for season in [Season::Summer, Season::Winter] {
            let eq = surface_temperature_c(&earth_at(0.0, 0.0), season, 12.0);
            let pole = surface_temperature_c(&earth_at(90.0, 0.0), season, 12.0);
            assert!(
                eq > pole + 10.0,
                "{season:?}: equator {eq} should be well above pole {pole}"
            );
        }
        // Hemispheres are symmetric in v1 (no seasonal flip yet).
        let north = surface_temperature_c(&earth_at(30.0, 0.0), Season::Summer, 12.0);
        let south = surface_temperature_c(&earth_at(-30.0, 0.0), Season::Summer, 12.0);
        assert_eq!(north, south);
    }

    /// Climbing 2 km costs 13 C on a body with air; airless bodies have
    /// no lapse at all; the lapse stops at the tropopause.
    #[test]
    fn altitude_lapse_only_with_atmosphere() {
        let sea = surface_temperature_c(&earth_at(45.0, 0.0), Season::Summer, 12.0);
        let high = surface_temperature_c(&earth_at(45.0, 2_000.0), Season::Summer, 12.0);
        assert!(
            (sea - high - 2.0 * LAPSE_RATE_C_PER_KM).abs() < 1e-4,
            "2 km should cost {} C, got {}",
            2.0 * LAPSE_RATE_C_PER_KM,
            sea - high
        );
        // Above the tropopause the drop is capped.
        let strato = surface_temperature_c(&earth_at(45.0, 30_000.0), Season::Summer, 12.0);
        let cap = surface_temperature_c(
            &earth_at(45.0, TROPOPAUSE_ALTITUDE_KM * 1000.0),
            Season::Summer,
            12.0,
        );
        assert_eq!(strato, cap);
        // Airless: same reading at any altitude (same hour).
        let moon = BodyEnvironment::airless("moon", 220.0);
        let mut moon_high = moon.clone();
        moon_high.altitude_m = 5_000.0;
        assert_eq!(
            surface_temperature_c(&moon, Season::Summer, 12.0),
            surface_temperature_c(&moon_high, Season::Summer, 12.0)
        );
    }

    /// The Moon swings brutally between its day and night, centered on
    /// its catalog mean; Earth has no diurnal term (its table carries it).
    #[test]
    fn moon_day_night_swing_is_large() {
        let moon = BodyEnvironment::airless("moon", 220.0);
        let day = surface_temperature_c(&moon, Season::Summer, DIURNAL_PEAK_HOUR);
        let night = surface_temperature_c(&moon, Season::Summer, DIURNAL_PEAK_HOUR + 12.0);
        assert!(
            day - night > 200.0,
            "airless swing should exceed 200 C, got {} ({day} vs {night})",
            day - night
        );
        // Centered on the catalog mean at the equator (plus the latitude
        // gradient's equator shift).
        assert!(day > 0.0, "lunar day should be above freezing, got {day}");
        assert!(night < -100.0, "lunar night should be brutal, got {night}");
        // Earth: same reading day and night.
        let e = earth_at(45.0, 0.0);
        assert_eq!(
            surface_temperature_c(&e, Season::Summer, 2.0),
            surface_temperature_c(&e, Season::Summer, 14.0)
        );
    }

    /// Airless bodies host no weather; dry atmospheres host no
    /// water-borne weather (rain/snow/fog remap, storms become dust).
    #[test]
    fn no_precipitation_on_airless_or_dry_worlds() {
        use WeatherCondition as WC;
        let moon = BodyEnvironment::airless("moon", 220.0);
        for cond in [
            WC::Clear,
            WC::Cloudy,
            WC::Rain,
            WC::Storm,
            WC::Snow,
            WC::Fog,
            WC::Sandstorm,
        ] {
            assert_eq!(sanitize_condition(cond, &moon), WC::Clear);
        }
        let mars = BodyEnvironment::dry_atmosphere("mars", 210.0);
        assert_eq!(sanitize_condition(WC::Rain, &mars), WC::Cloudy);
        assert_eq!(sanitize_condition(WC::Snow, &mars), WC::Cloudy);
        assert_eq!(sanitize_condition(WC::Fog, &mars), WC::Cloudy);
        assert_eq!(sanitize_condition(WC::Storm, &mars), WC::Sandstorm);
        assert_eq!(sanitize_condition(WC::Clear, &mars), WC::Clear);
        assert_eq!(sanitize_condition(WC::Sandstorm, &mars), WC::Sandstorm);
        // Earth passes everything through.
        let earth = earth_at(45.0, 0.0);
        assert_eq!(sanitize_condition(WC::Rain, &earth), WC::Rain);
        assert_eq!(sanitize_condition(WC::Snow, &earth), WC::Snow);
    }

    /// The outside-air mapping: space and airless = vacuum, breathable =
    /// Earth-like, unbreathable atmosphere = thin toxic CO2.
    #[test]
    fn outside_atmosphere_mapping() {
        // Home orbit default: locked false = open space = vacuum.
        let home = BodyEnvironment::default();
        assert_eq!(outside_atmosphere_for(&home).pressure_atm, 0.0);
        // Standing on Earth: full breathable air.
        let earth = earth_at(21.0, 3.0);
        let a = outside_atmosphere_for(&earth);
        assert!(a.pressure_atm > 0.9);
        assert!(a.gas_percent("O2") > 20.0);
        // The Moon: vacuum.
        let moon = BodyEnvironment::airless("moon", 220.0);
        assert_eq!(outside_atmosphere_for(&moon).pressure_atm, 0.0);
        // Mars: thin, oxygen-free.
        let mars = BodyEnvironment::dry_atmosphere("mars", 210.0);
        let m = outside_atmosphere_for(&mars);
        assert!(m.pressure_atm < 0.1);
        assert!(m.gas_percent("O2") < 1.0);
    }

    /// Review fix: the frame-lock -> environment mapping. A def-less
    /// catalog body publishes as an airless rock, never as the Earth
    /// default (the "it can rain on Venus" hole), and the catalog's 0.0
    /// absent-temperature sentinel falls back like an unknown id.
    #[test]
    fn frame_lock_mapping_covers_defless_bodies_and_bad_catalog_temps() {
        // No frame lock: the Earth-home default, exactly (home farming
        // and the home sky depend on this arm staying bit-identical).
        let home = environment_for_frame_lock(None, None, None, 12.0, 500.0);
        assert!(!home.locked, "home frame is not locked to a body");
        assert_eq!(home.body_id, "earth");
        assert!(home.has_atmosphere && home.has_water && home.breathable);

        // Locked to a catalog body with NO data/planets def (venus,
        // titan, io, ceres...): AIRLESS + waterless + unbreathable.
        let venus = environment_for_frame_lock(Some("venus"), None, Some(737.0), -30.0, 0.0);
        assert_eq!(venus.body_id, "venus");
        assert!(venus.locked);
        assert!(!venus.has_atmosphere, "def-less body must not inherit Earth air");
        assert!(!venus.has_water, "def-less body must not inherit Earth water");
        assert!(!venus.breathable);
        assert_eq!(venus.mean_temperature_k, 737.0);
        assert_eq!(venus.latitude_deg, -30.0);
        // And therefore no weather at all can be rolled there.
        assert_eq!(sanitize_condition(WeatherCondition::Rain, &venus), WeatherCondition::Clear);

        // Locked to a body WITH a def: the def decides everything.
        let def: PlanetDef = ron::from_str(
            r#"(
                name: "Testworld",
                radius: 6371000.0,
                gravity: 9.81,
                terrain_seed: 1,
                ore_seed: 1,
                atmosphere_color: Some((0.5, 0.7, 1.0, 1.0)),
                has_water: true,
                breathable: true,
            )"#,
        )
        .expect("minimal PlanetDef parses");
        let world =
            environment_for_frame_lock(Some("testworld"), Some(&def), Some(300.0), 10.0, 2000.0);
        assert!(world.locked && world.has_atmosphere && world.has_water && world.breathable);
        assert_eq!(world.mean_temperature_k, 300.0);
        assert_eq!(world.altitude_m, 2000.0);

        // The catalog's 0.0 absent-field sentinel is treated as unknown:
        // same 288 K fallback as an id the catalog does not know at all.
        let sentinel = environment_for_frame_lock(Some("mystery_rock"), None, Some(0.0), 0.0, 0.0);
        assert_eq!(sentinel.mean_temperature_k, 288.0, "0.0 sentinel must not be a temperature");
        let missing = environment_for_frame_lock(Some("mystery_rock"), None, None, 0.0, 0.0);
        assert_eq!(missing.mean_temperature_k, 288.0);
        // A real (if extreme) catalog value passes through untouched.
        let pluto = environment_for_frame_lock(Some("pluto"), None, Some(44.0), 0.0, 0.0);
        assert_eq!(pluto.mean_temperature_k, 44.0);
    }

    /// Review fix: the positional offset carries ONLY latitude +
    /// altitude. The day/night swing is body-global (one game clock)
    /// and rides the global temperature export instead, so farming on
    /// an airless body still sees the swing while never seeing the
    /// player's personal altitude.
    #[test]
    fn positional_offset_excludes_the_diurnal_swing() {
        let moon = BodyEnvironment::airless("moon", 220.0);
        // Hour-invariant: noon and midnight give the same positional delta.
        let noon = positional_temp_offset_c(&moon, Season::Summer, DIURNAL_PEAK_HOUR);
        let midnight = positional_temp_offset_c(&moon, Season::Summer, DIURNAL_PEAK_HOUR + 12.0);
        assert!(
            (noon - midnight).abs() < 1e-4,
            "day/night swing leaked into the positional delta: {noon} vs {midnight}"
        );
        // The swing itself is alive, global, and large on airless bodies.
        let day = diurnal_swing_c(&moon, DIURNAL_PEAK_HOUR);
        let night = diurnal_swing_c(&moon, DIURNAL_PEAK_HOUR + 12.0);
        assert!(day - night > 200.0, "airless swing should exceed 200 C, got {}", day - night);
        // Earth's calibrated table already carries its day cycle: zero swing.
        assert_eq!(diurnal_swing_c(&BodyEnvironment::default(), 3.0), 0.0);
        assert_eq!(diurnal_swing_c(&BodyEnvironment::default(), 14.0), 0.0);
        // Decomposition is lossless: baseline + swing + positional
        // reconstructs the full model at any position and hour.
        let mut high = moon.clone();
        high.latitude_deg = 60.0;
        high.altitude_m = 3000.0;
        let full = surface_temperature_c(&high, Season::Summer, 5.0);
        let rebuilt = body_baseline_temp_c(&high, Season::Summer)
            + diurnal_swing_c(&high, 5.0)
            + positional_temp_offset_c(&high, Season::Summer, 5.0);
        assert!((full - rebuilt).abs() < 1e-4, "split lost temperature: {full} vs {rebuilt}");
    }

    /// Outside-the-hull breathability: only on a locked, breathable body
    /// below the ceiling. Space, the Moon, Mars, and Everest-plus all say
    /// no.
    #[test]
    fn breathable_outside_rules() {
        assert!(!BodyEnvironment::default().breathable_outside(), "open space");
        assert!(earth_at(45.0, 0.0).breathable_outside(), "Earth sea level");
        let mut high = earth_at(45.0, 0.0);
        high.altitude_m = BREATHABLE_CEILING_M + 1.0;
        assert!(!high.breathable_outside(), "above the death zone");
        assert!(!BodyEnvironment::airless("moon", 220.0).breathable_outside());
        assert!(!BodyEnvironment::dry_atmosphere("mars", 210.0).breathable_outside());
    }
}

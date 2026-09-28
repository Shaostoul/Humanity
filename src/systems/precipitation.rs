//! What falls from the sky, and in which phase (2026-09-27).
//!
//! The weather's condition says WHETHER water falls and how hard: Rain, Storm
//! and Snow at their intensity, nothing for the rest. The AIR where it falls
//! says whether it arrives as rain or as snow, so one weather system rains on
//! the shore and snows on the summit above it, and a Rain roll at the pole in
//! winter falls as snow. The condition still names the weather SYSTEM (its
//! clouds, its cold deviation); it no longer decides the phase.
//!
//! Every reader of precipitation goes through here so they agree on the phase
//! (docs/design/environment-fields.md, "Rain or snow, decided by the air"):
//!
//! - the body heat model's wetting input, through `engine::survival_env`
//!   (`Weather::falling_at_player`, rain at full weight, snow at a third);
//! - the HUD weather line, the weather fog and the F11 panel's readback,
//!   through the bridged condition (`Weather::condition_at_player`) and the
//!   bridged `Falling`;
//! - the rain and snow particle emitters (lib.rs, `Falling::emitters`);
//! - rain watering outdoor fields (farming) and the water bodies (hydrology),
//!   which read the BODY-GLOBAL reference air like the rest of their climate
//!   (`Weather::falling_global`), so a player's climb cannot freeze a field.
//!
//! THE SOURCE. Jennings, K. S., Winchell, T. S., Livneh, B. and Molotch, N. P.
//! (2018), "Spatial variation of the rain-snow temperature threshold across
//! the Northern Hemisphere", Nature Communications 9:1148,
//! doi:10.1038/s41467-018-03629-7 (open access; read 2026-09-27). From 17.8
//! million observations at 11,924 stations, 1978-2007: rain and snow fall with
//! equal frequency at an air temperature "averaging 1.0 °C and ranging from
//! –0.4 to 2.4 °C for 95% of the stations", and humidity moves it: "0.7 °C in
//! the 90–100% RH bin to 4.5 °C in the 40–50% RH bin", because dry air cools a
//! falling flake by evaporation and keeps it frozen in above-freezing air (the
//! wet-bulb effect). Their trivariate logistic model (Methods, equation 4)
//!
//!   p(snow) = 1 / (1 + exp(alpha + beta Ts + gamma RH + lambda Ps))
//!
//! with Ts in C, RH in percent and Ps in kPa, and the coefficients from their
//! Supplementary Table 2: alpha -12.80, beta 1.41, gamma 0.09, lambda 0.03.
//! It was one of the paper's two best methods, and every input it wants is
//! already at the player: Layer 1's temperature and pressure and the weather's
//! humidity. At 90 percent humidity and sea level its 50 percent point is
//! 1.18 C, and the share goes from nine tenths snow to nine tenths rain over
//! about 3 C (ln 9 / 1.41 either side): a smooth band, not a switch.
//!
//! TWO READINGS OF OURS, not the paper's. (1) The paper fits how OFTEN snow
//! falls; we read p(snow) as the SHARE of what falls that is frozen, which
//! gives mixed rain and snow inside the band rather than a coin toss. (2) The
//! fit is Northern Hemisphere land stations between 60 and 105 kPa and 10 to
//! 100 percent humidity; we clamp the inputs to that range and apply it over
//! the sea and in the south too, where it was not fitted.

use crate::systems::weather::{Weather, WeatherCondition};

/// Jennings et al. 2018, Supplementary Table 2, trivariate model: the
/// intercept.
const ALPHA: f32 = -12.80;
/// Per degree C of air temperature.
const BETA_PER_C: f32 = 1.41;
/// Per percent of relative humidity.
const GAMMA_PER_RH_PCT: f32 = 0.09;
/// Per kPa of surface pressure.
const LAMBDA_PER_KPA: f32 = 0.03;
/// The pressure the model was fitted over, kPa (the paper's four bins run from
/// 60 to 105): a Himalayan summit reads as 60, never extrapolated.
const FIT_KPA: (f32, f32) = (60.0, 105.0);
/// The humidity kept in the fit, percent (the paper dropped records below 10
/// and above 100).
const FIT_RH_PCT: (f32, f32) = (10.0, 100.0);
/// Sea-level pressure, kPa (the 1976 US Standard Atmosphere): the column the
/// body-global reference temperature stands for.
pub const SEA_LEVEL_KPA: f32 = 101.325;
/// A DISPLAY CHOICE: a phase is named on the HUD once it is at least a fifth of
/// what falls, so "rain and snow" covers the middle of the band (about 2 C
/// wide at 90 percent humidity).
const NAMED_SHARE: f32 = 0.2;

/// The share of falling precipitation that is snow, 0 (all rain) to 1 (all
/// snow), in air at `temp_c` with relative humidity `rh` (0 to 1) and pressure
/// `pressure_kpa`. Jennings et al. 2018's trivariate model (module docs).
pub fn snow_share(temp_c: f32, rh: f32, pressure_kpa: f32) -> f32 {
    let rh_pct = (rh * 100.0).clamp(FIT_RH_PCT.0, FIT_RH_PCT.1);
    let ps = pressure_kpa.clamp(FIT_KPA.0, FIT_KPA.1);
    let z = ALPHA + BETA_PER_C * temp_c + GAMMA_PER_RH_PCT * rh_pct + LAMBDA_PER_KPA * ps;
    1.0 / (1.0 + z.exp())
}

/// What is falling at one place, each 0 (none) to 1 (a downpour), and the two
/// together never more than 1.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Falling {
    /// Liquid water.
    pub rain: f32,
    /// Frozen water.
    pub snow: f32,
}

impl Falling {
    /// Nothing falling.
    pub const NONE: Falling = Falling { rain: 0.0, snow: 0.0 };

    /// How hard it falls, whatever the phase.
    pub fn total(self) -> f32 {
        self.rain + self.snow
    }

    /// Mostly snow (the dominant phase). False when nothing falls.
    pub fn is_snow(self) -> bool {
        self.snow > self.rain
    }

    /// What the HUD calls it: "" when nothing falls, else "rain", "snow" or,
    /// inside the band, "rain and snow".
    pub fn phase_word(self) -> &'static str {
        let t = self.total();
        if t <= 0.0 {
            return "";
        }
        let s = self.snow / t;
        if s < NAMED_SHARE {
            "rain"
        } else if s > 1.0 - NAMED_SHARE {
            "snow"
        } else {
            "rain and snow"
        }
    }

    /// Which precipitation emitters the particles run, (rain, snow): the
    /// dominant phase only. The GPU particle path simulates one pool, so a
    /// mixed band draws whichever phase is the larger share; drawing both at
    /// their shares is the next rung (per-emitter rates in lib.rs's
    /// precipitation block and a second GPU pool).
    pub fn emitters(self) -> (bool, bool) {
        if self.total() <= 0.0 {
            (false, false)
        } else {
            (!self.is_snow(), self.is_snow())
        }
    }
}

/// What falls from a weather `condition` at `intensity` through air at
/// `temp_c`, relative humidity `rh` (0 to 1) and `pressure_kpa`. Rain, Storm
/// and Snow bring water at their intensity; the air splits it into rain and
/// snow. No air (pressure 0: open space, an airless world, outside the hull of
/// a station in orbit) means nothing falls here, whatever the weather of the
/// world below is doing.
pub fn falling(condition: WeatherCondition, intensity: f32, temp_c: f32, rh: f32, pressure_kpa: f32) -> Falling {
    use WeatherCondition::*;
    let i = match condition {
        Rain | Storm | Snow => intensity.clamp(0.0, 1.0),
        Clear | Cloudy | Fog | Sandstorm => 0.0,
    };
    if i <= 0.0 || !(pressure_kpa > 0.0) {
        return Falling::NONE;
    }
    let s = snow_share(temp_c, rh, pressure_kpa);
    Falling { rain: i * (1.0 - s), snow: i * s }
}

impl Weather {
    /// What falls AT THE PLAYER: the condition's water, in the phase the air
    /// where the player stands decides (the at-player temperature and
    /// pressure: environment Layer 1 plus the weather's deviation). The body
    /// heat model, the HUD, the weather fog and the particles read this. Valid
    /// on the EXPORTED weather (the DataStore's "weather"), where the at-player
    /// fields are filled.
    pub fn falling_at_player(&self) -> Falling {
        falling(self.condition, self.intensity, self.temperature_at_player, self.humidity, self.pressure_kpa_at_player)
    }

    /// What falls in the BODY-GLOBAL reference air (the `temperature` farming
    /// and hydrology read, at sea level). Wherever fields exist today (the home
    /// frame) this is the same air as the player's; the two part only when the
    /// player stands on a world away from the fields, where it can rightly
    /// snow on the player while it rains on the crops.
    pub fn falling_global(&self) -> Falling {
        falling(self.condition, self.intensity, self.temperature, self.humidity, SEA_LEVEL_KPA)
    }

    /// The condition as it is felt at the player: a Rain or Snow condition
    /// named by the phase that actually falls there (a Rain system in freezing
    /// air is Snow). Every other condition, and a Rain or Snow condition where
    /// nothing falls on the player, is unchanged. The HUD label and icon and
    /// the weather fog (its visibility floor and tint) key off this.
    pub fn condition_at_player(&self) -> WeatherCondition {
        use WeatherCondition::*;
        match self.condition {
            Rain | Snow => {
                let f = self.falling_at_player();
                if f.total() <= 0.0 {
                    self.condition
                } else if f.is_snow() {
                    Snow
                } else {
                    Rain
                }
            }
            c => c,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::body_environment::BodyEnvironment;
    use crate::systems::env_layer1::ClimateTable;
    use crate::systems::time::Season;
    use crate::systems::weather::{air_at_player, AtPlayerInputs};

    /// THE BAND IS SMOOTH, AND SITS WHERE THE PAPER PUTS IT. At 90 percent
    /// humidity and sea level the 50 percent point is 1.18 C (from the
    /// coefficients: (12.80 - 8.10 - 3.04) / 1.41); 2 C colder is almost all
    /// snow, 3 C warmer almost all rain; the share falls monotonically and
    /// never jumps (no 0.1 C step moves it more than the logistic's steepest
    /// slope allows, beta / 4 = 0.035 per 0.1 C); and drier air holds the
    /// snow to a warmer temperature, as the paper observed (0.7 C at 90-100
    /// percent, 4.5 C at 40-50). Seen red: a hard switch at 0 C (the share 1
    /// below freezing, 0 above) fails at the centre, the first assertion (the
    /// 1 C sample and the smoothness check behind it would fail too).
    #[test]
    fn the_rain_snow_band_is_smooth_and_centred_where_jennings_puts_it() {
        let wet = |t: f32| snow_share(t, 0.9, SEA_LEVEL_KPA);
        assert!((wet(1.1775) - 0.5).abs() < 0.01, "50% point at 90% RH: {}", wet(1.1775));
        assert!(wet(-1.0) > 0.95, "freezing air snows: {}", wet(-1.0));
        assert!(wet(4.5) < 0.05, "warm air rains: {}", wet(4.5));
        assert!(wet(1.0) > 0.4 && wet(1.0) < 0.7, "1 C is inside the band: {}", wet(1.0));
        let mut prev = wet(-6.0);
        for k in 1..=140 {
            let s = wet(-6.0 + k as f32 * 0.1);
            assert!(s <= prev, "monotonic at {} C", -6.0 + k as f32 * 0.1);
            assert!(prev - s < 0.036, "no jump at {} C: {prev} -> {s}", -6.0 + k as f32 * 0.1);
            prev = s;
        }
        // Dry air: the same 50 percent point sits several degrees warmer.
        let t50 = |rh: f32| (-ALPHA - GAMMA_PER_RH_PCT * rh * 100.0 - LAMBDA_PER_KPA * SEA_LEVEL_KPA) / BETA_PER_C;
        assert!(t50(0.45) > t50(0.95) + 2.5, "dry {} vs saturated {}", t50(0.45), t50(0.95));
        assert!((snow_share(t50(0.45), 0.45, SEA_LEVEL_KPA) - 0.5).abs() < 1e-3);
        // Thin air, a little warmer (0.03 / 1.41 C per kPa), clamped at the fit's edge.
        assert!(snow_share(2.0, 0.9, 70.0) > wet(2.0));
        assert_eq!(snow_share(2.0, 0.9, 30.0), snow_share(2.0, 0.9, 60.0));
    }

    /// Earth, locked, standing at `lat` degrees, `alt` metres, deep inland.
    fn earth(lat: f64, alt: f32) -> BodyEnvironment {
        let la = lat.to_radians();
        BodyEnvironment {
            locked: true,
            latitude_deg: lat as f32,
            altitude_m: alt,
            up_dir: glam::DVec3::new(la.cos(), la.sin(), 0.0),
            land_fraction: 1.0,
            ..Default::default()
        }
    }

    /// The exported weather at `env` on `year_fraction`, under `condition`:
    /// Layer 1 at the player (no deviation), 90 percent humidity.
    fn weather_at(env: &BodyEnvironment, year_fraction: f64, condition: WeatherCondition) -> Weather {
        let at = air_at_player(&AtPlayerInputs {
            env,
            table: ClimateTable::shipped(),
            season: Season::Winter,
            hour: 12.0,
            year_fraction,
            global_c: 5.0,
            temp_dev_c: 0.0,
            weather_wind: (0.0, 0.0),
            manual_wind: false,
        });
        Weather {
            condition,
            intensity: 0.8,
            humidity: 0.9,
            temperature_at_player: at.temp_c,
            pressure_kpa_at_player: at.pressure_kpa,
            ..Default::default()
        }
    }

    /// LAYER 1 DECIDES WHAT FALLS. At 70 N inland at the northern winter
    /// solstice (year fraction 0.75 on Earth's row) Layer 1 puts the air well
    /// below freezing, and a RAIN condition falls there as snow; at the
    /// equator a SNOW condition falls as rain; walking up a mountain at 35 N in
    /// winter carries the player from rain through the band into snow with no
    /// jump; and outside the air (a station in orbit) nothing falls. Seen red:
    /// with the condition deciding the phase (Rain all rain, Snow all snow, the
    /// code before this module) the 70 N arm fails on "a Rain roll in ... C
    /// air falls as snow".
    #[test]
    fn layer_1_decides_whether_it_rains_or_snows() {
        let winter = 0.75;
        let arctic = weather_at(&earth(70.0, 0.0), winter, WeatherCondition::Rain);
        assert!(arctic.temperature_at_player < -10.0, "Layer 1 at 70 N in winter: {} C", arctic.temperature_at_player);
        let f = arctic.falling_at_player();
        assert!(f.snow > 0.79 && f.rain < 0.01, "a Rain roll in {} C air falls as snow: {f:?}", arctic.temperature_at_player);
        assert_eq!(arctic.condition_at_player(), WeatherCondition::Snow);
        assert_eq!(f.phase_word(), "snow");

        let tropic = weather_at(&earth(0.0, 0.0), winter, WeatherCondition::Snow);
        assert!(tropic.temperature_at_player > 20.0, "{}", tropic.temperature_at_player);
        let f = tropic.falling_at_player();
        assert!(f.rain > 0.79 && f.snow < 0.01, "a Snow roll at the equator falls as rain: {f:?}");
        assert_eq!(tropic.condition_at_player(), WeatherCondition::Rain);

        // The snow line: climbing in 50 m steps from the shore at 35 N.
        let share = |alt: f32| {
            let w = weather_at(&earth(35.0, alt), winter, WeatherCondition::Rain);
            let f = w.falling_at_player();
            (f.snow / f.total(), w.temperature_at_player)
        };
        let (shore, shore_c) = share(0.0);
        let (summit, summit_c) = share(5_000.0);
        assert!(shore < 0.05 && shore_c > 5.0, "rain on the shore at {shore_c} C: {shore}");
        assert!(summit > 0.95 && summit_c < -5.0, "snow on the summit at {summit_c} C: {summit}");
        let mut prev = shore;
        let mut mixed = false;
        for k in 1..=100 {
            let (s, _) = share(k as f32 * 50.0);
            assert!(s >= prev - 1e-6 && s - prev < 0.15, "smooth at {} m: {prev} -> {s}", k * 50);
            mixed |= s > 0.3 && s < 0.7;
            prev = s;
        }
        assert!(mixed, "the climb passes through rain and snow together");

        // No air, nothing falls: the home station's weather is the world below's.
        let mut orbit = arctic.clone();
        orbit.pressure_kpa_at_player = 0.0;
        assert_eq!(orbit.falling_at_player(), Falling::NONE);
        assert_eq!(orbit.condition_at_player(), WeatherCondition::Rain, "nothing falls here, the label stays");
        // And a condition that brings no water brings none in any air.
        assert_eq!(weather_at(&earth(70.0, 0.0), winter, WeatherCondition::Fog).falling_at_player(), Falling::NONE);
    }

    /// The HUD words and the emitters follow the shares.
    #[test]
    fn the_phase_names_and_the_emitters() {
        assert_eq!(Falling::NONE.phase_word(), "");
        assert_eq!(Falling::NONE.emitters(), (false, false));
        assert_eq!(Falling { rain: 0.5, snow: 0.05 }.phase_word(), "rain");
        assert_eq!(Falling { rain: 0.3, snow: 0.3 }.phase_word(), "rain and snow");
        assert_eq!(Falling { rain: 0.02, snow: 0.6 }.phase_word(), "snow");
        assert_eq!(Falling { rain: 0.2, snow: 0.3 }.emitters(), (false, true));
        assert_eq!(Falling { rain: 0.3, snow: 0.2 }.emitters(), (true, false));
    }
}

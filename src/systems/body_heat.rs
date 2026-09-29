//! Body heat (2026-09-27): how warm a person stays, from the heat they make and
//! the heat the weather, their clothes and their shelter take away.
//!
//! WHAT IT REPLACED. The core temperature used to move straight toward the
//! air temperature at 0.5 C a second, with no body heat, clothes, wind, wet
//! or sun in it, so a person stepping outside on a 15 C day crossed 35 C
//! (hypothermia) in four and a half seconds and died about fifty seconds
//! later. A real person in ordinary clothes is comfortable at 15 C all day.
//!
//! THE MODEL is the Gagge two-node model: Gagge, Stolwijk and Nishi 1971, "An
//! effective temperature scale based on a simple model of human physiological
//! regulatory response", ASHRAE Transactions 77(1):247-262, in the form of
//! Gagge, Fobelets and Berglund 1986, "A standard predictive index of human
//! response to the thermal environment", ASHRAE Transactions 92(2B):709-731.
//! That form is the one ASHRAE Standard 55 and ASHRAE Handbook Fundamentals
//! chapter 9 use, and the one the Center for the Built Environment's
//! open-source `pythermalcomfort` codes as `two_nodes_gagge` (read 2026-09-27);
//! every coefficient below is that code's, and the names say which is which.
//! The body is two shells: a CORE and a SKIN layer. Metabolism heats the core;
//! blood carries heat to the skin; the skin loses it through the clothes to
//! the air (convection), to the surroundings (radiation) and by sweat
//! evaporating (and the lungs lose some breathing). The body answers with
//! the three responses Gagge coded: skin blood flow (narrowing in the cold,
//! widening in the heat), sweating, and shivering.
//!
//! WHAT IS ADDED to Gagge, each with its source, because Gagge built the model
//! for rooms and a game needs a person to be able to be caught out in the cold:
//! - Shivering has a ceiling and it tires. Peak shivering is 4.9 times the
//!   resting metabolism (Eyolfson, Tikuisis, Xu, Weseen and Giesbrecht 2001,
//!   "Measurement and prediction of peak shivering intensity in humans", Eur
//!   J Appl Physiol 84:100-106). Past its endurance the shivering drive falls
//!   by about 17 percent an hour (Tikuisis, Eyolfson, Xu and Giesbrecht 2002,
//!   "Shivering endurance and fatigue during cold water immersion in humans",
//!   Eur J Appl Physiol 87:50-58). Shivering fades out as the core passes 32
//!   to 30 C (StatPearls, "Hypothermia", as cited in the Library's
//!   cold_and_hypothermia guide).
//! - Wet clothes. Soaked clothing keeps about 70 percent of its dry
//!   insulation ("almost 30%" lower: Zhao, Yu, Niu, Zhou and Fan 2025, Building
//!   and Environment 267:112299), and the water evaporating out of it cools
//!   the body with a fraction of the latent heat: Havenith et al. 2013, "Evaporative
//!   cooling: effective latent heat of evaporation in relation to evaporation
//!   distance from the skin", J Appl Physiol 114:778-785, measured a 28 percent
//!   loss for a wet base layer and more than 62 percent for a wet outer layer.
//! - Weather wind is measured at 10 m; a person feels about two thirds of it
//!   (the reduction the 2001 wind chill chart uses: Osczevski and Bluestein
//!   2005, "The new wind chill equivalent temperature chart", BAMS 86:1453).
//!
//! CALIBRATED, NOT PUBLISHED: the shivering endurance (`SHIVER_ENDURANCE`,
//! `SHIVER_ENDURANCE_EXPONENT`) and the sustained shivering ceiling
//! (`SHIVER_SUSTAINED_SHARE`). They were set so the model reproduces, together:
//! the Cold Exposure Survival Model's predicted survival for a still person
//! with no clothes in calm air, 9.0 h at 0 C and more than 24 h at 10 C
//! (Tikuisis 1995, "Predicting survival time for cold exposure", Int J
//! Biometeorol 39:94-102; survival = the core reaching 28 C); the 0.25 C fall
//! Helland et al. measured over 3 h in wet light clothes at 5 C in a wind
//! (Wilderness Environ Med 2025, doi 10.1177/10806032251378099); and Thompson
//! and Hayward's walkers holding their core for 4 h of 5 C rain (J Appl Physiol
//! 1996, 81:1128-1137). The model reaches 28 C in 9.6 h at 0 C (CESM 9.0 h),
//! 24.7 h at 10 C (CESM more than 24 h). In harder cold it is slower than the
//! CESM (5.5 h against 4.1 h at -10 C, 4.0 h against 2.5 h at -20 C): the two-node
//! model has one skin layer and no limbs, which is its known limit in severe cold.
//!
//! TWO MODES (the house rule for deep systems, CLAUDE.md "Dual modes"):
//! Realistic shows the model's core as it is. Forgiving, the default, runs the
//! same physics, so clothes, shelter, wind and wet all count exactly as much,
//! but shows the core swinging HALF as far from normal and applies the harm at
//! half the rate: the garden's Gentle mode is the same "half" rule. Because the
//! Forgiving core is always closer to normal and its harm is halved, Forgiving
//! can never kill sooner than Realistic; a test holds that on a grid anyway.
//!
//! Everything here is pure data and math with no engine state, so it compiles
//! in every feature set (the relay build includes `systems/`).

use crate::ecs::components::EnvironmentContext;
use crate::hot_reload::data_store::DataStore;
use crate::systems::precipitation::Falling;

// -- Medical thresholds (the core temperature, C) ------------------------------------
//
// The cold bands are the ones the Library's cold_and_hypothermia guide cites:
// CDC ("a body temperature below 95 F (35 C) needs medical attention") and
// StatPearls (mild 32-35, moderate 28-32, severe below 28). The heat ones are
// the Merck Manual Professional's: heat exhaustion usually below 40 C with no
// change in mental state, heatstroke above 40 C with altered mental status.

/// Below this the person is hypothermic (CDC; StatPearls "mild").
pub const HYPOTHERMIA_C: f32 = 35.0;
/// Moderate hypothermia (StatPearls): lasting harm starts here in the game.
pub const MODERATE_HYPOTHERMIA_C: f32 = 32.0;
/// Severe hypothermia (StatPearls), and the Cold Exposure Survival Model's
/// end of survival time.
pub const SEVERE_HYPOTHERMIA_C: f32 = 28.0;
/// Overheating slows a person from here. A GAME ONSET inside the Merck heat
/// exhaustion range: hard exercise alone puts a healthy core at 38 to 39
/// (Thompson and Hayward's walkers rose to 38.1 C in their first hour).
pub const HEAT_EXHAUSTION_C: f32 = 39.0;
/// Heatstroke (Merck Manual Professional: above 40 C): lasting harm from here.
pub const HEAT_STROKE_C: f32 = 40.0;

/// Health a 100-point body loses per second per degree below 32 C. A GAME
/// CHOICE on the StatPearls bands: 100 points over 4 h a degree into moderate
/// hypothermia, over 1 h at 28 C (severe), 30 minutes at 24 C.
pub const COLD_HARM_PER_DEG_S: f32 = 100.0 / (4.0 * 3600.0);
/// Health lost per second per degree above 40 C (heatstroke): a GAME CHOICE,
/// 100 points in an hour at 41 C, 30 minutes at 42 C, 20 minutes at 43 C.
pub const HEAT_HARM_PER_DEG_S: f32 = 100.0 / 3600.0;

// -- Modes ---------------------------------------------------------------------------

/// DataStore key of the Settings mode (a plain `Mode`), published each frame
/// by `engine::survival_env` from Settings > Gameplay > Body heat.
pub const MODE_KEY: &str = "body_heat_mode";

/// The two modes of the house rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// The default: the same physics, the core shown swinging half as far
    /// from normal, harm at half the rate.
    #[default]
    Forgiving,
    /// The model's core as it is, harm at the full rate.
    Realistic,
}

/// How far from normal the Forgiving core swings, as a share of the real one.
pub const FORGIVING_SWING: f64 = 0.5;
/// The Forgiving harm rate, as a share of the Realistic one.
pub const FORGIVING_HARM: f32 = 0.5;

impl Mode {
    /// The Settings mode. Absent (headless tests, the relay) is Realistic:
    /// the physics the tests measure, the same convention ship life support uses.
    pub fn from_store(data: &DataStore) -> Self {
        data.get::<Mode>(MODE_KEY).copied().unwrap_or(Mode::Realistic)
    }

    /// The core temperature this mode shows for a real one.
    pub fn shown(self, real_core_c: f64) -> f64 {
        match self {
            Mode::Realistic => real_core_c,
            Mode::Forgiving => CORE_NEUTRAL_C + FORGIVING_SWING * (real_core_c - CORE_NEUTRAL_C),
        }
    }

    /// The real core temperature a shown one stands for (a loaded save, a respawn).
    pub fn real(self, shown_core_c: f64) -> f64 {
        match self {
            Mode::Realistic => shown_core_c,
            Mode::Forgiving => CORE_NEUTRAL_C + (shown_core_c - CORE_NEUTRAL_C) / FORGIVING_SWING,
        }
    }
}

/// Health lost per second (of a 100-point body) at a SHOWN core temperature.
/// Nothing between 32 and 40 C: mild hypothermia and heat exhaustion slow a
/// person (their status effects) but do no lasting harm.
pub fn harm_per_s(shown_core_c: f32, mode: Mode) -> f32 {
    let h = if shown_core_c < MODERATE_HYPOTHERMIA_C {
        (MODERATE_HYPOTHERMIA_C - shown_core_c) * COLD_HARM_PER_DEG_S
    } else if shown_core_c > HEAT_STROKE_C {
        (shown_core_c - HEAT_STROKE_C) * HEAT_HARM_PER_DEG_S
    } else {
        0.0
    };
    match mode {
        Mode::Realistic => h,
        Mode::Forgiving => h * FORGIVING_HARM,
    }
}

// -- Activity (met; 1 met = 58.2 W per square metre of skin) -------------------------
//
// ASHRAE Handbook Fundamentals chapter 9, Table 4 (typical metabolic heat
// generation): sleeping 0.7, standing relaxed 1.2, walking on the level at
// 0.9 m/s 2.0, at 1.8 m/s 3.8; driving a car 1.0 to 2.0. The game's walking
// pace is faster than a real walk (a pacing choice), so its gait is billed
// by what the player is doing, not by the speed on screen.

/// Asleep (ASHRAE: sleeping, 0.7 met).
pub const MET_SLEEPING: f32 = 0.7;
/// Standing still (ASHRAE: standing, relaxed, 1.2 met).
pub const MET_STANDING: f32 = 1.2;
/// Walking (ASHRAE: walking on the level at 0.9 m/s, 2.0 met).
pub const MET_WALKING: f32 = 2.0;
/// Sprinting (ASHRAE: walking at 1.8 m/s, 3.8 met; heavy work is 3 to 4).
pub const MET_SPRINTING: f32 = 3.8;
/// Driving (ASHRAE: automobile, 1.0 to 2.0 met; the middle).
pub const MET_DRIVING: f32 = 1.5;

/// The player's activity from their movement state.
pub fn activity_met(moving: bool, sprinting: bool) -> f32 {
    match (moving, sprinting) {
        (true, true) => MET_SPRINTING,
        (true, false) => MET_WALKING,
        _ => MET_STANDING,
    }
}

// -- Clothing (clo; 1 clo = 0.155 m2 K/W) ----------------------------------------------

/// The everyday clothes the player always has on and the Outfit does not list:
/// trousers and a long-sleeve shirt, with briefs, socks and shoes. ASHRAE
/// Handbook Fundamentals chapter 9 / ASHRAE Standard 55 table of typical
/// ensembles gives that 0.61 clo. Worn gear adds its own `clo`
/// (data/equipment.csv); ensemble insulation is summed garment by garment, the
/// method those tables are built for.
pub const BASE_OUTFIT_CLO: f32 = 0.61;

// -- The Gagge two-node constants (pythermalcomfort `two_nodes_gagge`) ------------------

/// Body mass, kg, and skin area, m2 (the model's standard person).
const BODY_MASS_KG: f64 = 70.0;
const BODY_AREA_M2: f64 = 1.8258;
/// One met, W/m2.
const MET_W_M2: f64 = 58.2;
/// Stefan-Boltzmann constant.
const SIGMA: f64 = 5.6697e-8;
/// Skin emissivity and the share of the skin that radiates (standing: 0.73).
const EMISSIVITY: f64 = 0.95;
const RADIATING_SHARE: f64 = 0.73;
/// Neutral set points, C: skin, core, and the mass-weighted body (0.1 skin).
const SKIN_NEUTRAL_C: f64 = 33.7;
/// Neutral core, C: where a comfortable person's core settles.
pub const CORE_NEUTRAL_C: f64 = 36.8;
const BODY_NEUTRAL_C: f64 = 0.1 * SKIN_NEUTRAL_C + 0.9 * CORE_NEUTRAL_C;
/// Skin blood flow, L per m2 per hour: neutral, least and most.
const BLOOD_FLOW_NEUTRAL: f64 = 6.3;
const BLOOD_FLOW_MIN: f64 = 0.5;
const BLOOD_FLOW_MAX: f64 = 90.0;
/// Control gains: sweating (per K of body warmth), widening (per K of core
/// warmth), narrowing (per K of skin cold), shivering (per K of skin cold x
/// K of core cold, W/m2).
const SWEAT_GAIN: f64 = 170.0;
const DILATE_GAIN: f64 = 120.0;
const CONSTRICT_GAIN: f64 = 0.5;
const SHIVER_GAIN: f64 = 19.4;
/// Most sweat a body makes, g per m2 per hour.
const SWEAT_MAX: f64 = 500.0;
/// Heat capacity of body tissue, W h per kg per K.
const TISSUE_HEAT_CAPACITY: f64 = 0.97;
/// The latent heat of evaporating water, J/kg (Havenith et al. 2013: 2,430 J/g).
const LATENT_HEAT_J_KG: f64 = 2.43e6;
/// Still air: the model's floor on air speed, m/s.
const STILL_AIR_M_S: f64 = 0.1;
/// Share of the 10 m weather wind a standing person feels (wind chill chart).
const WIND_AT_BODY_SHARE: f64 = 2.0 / 3.0;

// -- The additions (see the module doc) ----------------------------------------------

/// Peak shivering, as a multiple of resting metabolism (Eyolfson et al. 2001).
const SHIVER_PEAK_X_RESTING: f64 = 4.9;
/// The shivering heat at that peak, W/m2 (on top of the 1 met at rest).
const SHIVER_PEAK_W_M2: f64 = (SHIVER_PEAK_X_RESTING - 1.0) * MET_W_M2;
/// CALIBRATED: the share of the peak a person sustains out in the cold.
const SHIVER_SUSTAINED_SHARE: f64 = 0.4;
/// CALIBRATED: shivering endurance, in hours at the full peak; lighter
/// shivering lasts as the cube of how much lighter it is (the intensity
/// dependence Tikuisis et al. 2002 report: moderate shivering endures longer).
const SHIVER_ENDURANCE: f64 = 0.1;
const SHIVER_ENDURANCE_EXPONENT: f64 = 3.0;
/// Past its endurance the shivering drive falls this share an hour (Tikuisis et al. 2002).
const SHIVER_FATIGUE_PER_H: f64 = 0.17;
/// A GAME CHOICE: warm and not shivering, the tiredness passes in 4 hours.
const SHIVER_RECOVERY_PER_H: f64 = 0.25;
/// Shivering stops between these core temperatures (StatPearls: about 30 to 32 C).
const SHIVER_STOPS_FROM_C: f64 = 32.0;
const SHIVER_STOPPED_C: f64 = 30.0;
/// Shivering this hard (W/m2, a tenth of the peak) is shown as the `shivering` condition.
pub const SHIVERING_SHOWN_W_M2: f64 = 0.1 * SHIVER_PEAK_W_M2;
/// Soaked clothing's insulation loss (Zhao et al. 2025: "almost 30%").
const WET_INSULATION_LOSS: f64 = 0.3;
/// Share of the latent heat of water drying out of wet clothes that is drawn
/// from the body: between Havenith et al.'s 0.72 (wet base layer) and 0.38
/// (wet outer layer).
const WET_CLOTHES_COOLING_SHARE: f64 = 0.5;
/// AN ESTIMATE: water soaked clothing holds, kg per clo (a 0.6 clo outfit
/// holds about 0.7 kg), and never less than 0.3 kg.
const WATER_KG_PER_CLO: f64 = 1.2;
const WATER_KG_MIN: f64 = 0.3;
/// A GAME CHOICE: heavy rain (precipitation 1.0) soaks clothes in 15 minutes.
const SOAK_S: f64 = 900.0;
/// Integration step ceiling, s. The skin layer's time constant is about a
/// minute at full blood flow, so ten seconds keeps the Euler step accurate;
/// a frame is far shorter and takes one step.
const MAX_STEP_S: f64 = 10.0;

// -- Inputs --------------------------------------------------------------------------

/// What the weather, the room and the shelter are doing to the body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Exposure {
    /// Air temperature, C.
    pub air_c: f32,
    /// Mean radiant temperature, C: the surroundings the skin radiates to.
    pub radiant_c: f32,
    /// Relative humidity, 0 to 1.
    pub relative_humidity: f32,
    /// Wind, m/s, as the weather reports it (at 10 m).
    pub wind_10m_m_s: f32,
    /// Air pressure, kPa.
    pub pressure_kpa: f32,
    /// Rain or snow landing on the person, 0 (none) to 1 (a downpour).
    pub precipitation: f32,
    /// Under a roof and out of the wind: still air and nothing falling.
    pub sheltered: bool,
}

impl Exposure {
    /// Indoor still air at `air_c` and `relative_humidity`.
    pub fn indoors(air_c: f32, relative_humidity: f32) -> Self {
        Self {
            air_c,
            radiant_c: air_c,
            relative_humidity,
            wind_10m_m_s: 0.0,
            pressure_kpa: 101.325,
            precipitation: 0.0,
            sheltered: true,
        }
    }

    /// Outdoors at `air_c`, `relative_humidity`, a 10 m wind and precipitation.
    pub fn outdoors(air_c: f32, relative_humidity: f32, wind_10m_m_s: f32, precipitation: f32) -> Self {
        Self {
            air_c,
            radiant_c: air_c,
            relative_humidity,
            wind_10m_m_s,
            pressure_kpa: 101.325,
            precipitation,
            sheltered: false,
        }
    }

    /// From the survival context the main loop publishes. Inside a sealed
    /// space the air is still and nothing falls, whatever the weather is doing.
    pub fn from_context(env: &EnvironmentContext) -> Self {
        let indoors = env.sealed || env.sheltered;
        Self {
            air_c: env.ambient_temp_c,
            radiant_c: env.radiant_temp_c.unwrap_or(env.ambient_temp_c),
            relative_humidity: env.relative_humidity.clamp(0.0, 1.0),
            wind_10m_m_s: if indoors { 0.0 } else { env.wind_m_s.max(0.0) },
            pressure_kpa: env.pressure_kpa,
            precipitation: if indoors { 0.0 } else { env.precipitation.clamp(0.0, 1.0) },
            sheltered: indoors,
        }
    }

    /// The air speed at the body, m/s: still air under shelter.
    fn air_speed(&self) -> f64 {
        if self.sheltered {
            STILL_AIR_M_S
        } else {
            (f64::from(self.wind_10m_m_s) * WIND_AT_BODY_SHARE).max(STILL_AIR_M_S)
        }
    }
}

/// How much falling snow wets clothing, against rain at the same rate: A GAME
/// CHOICE, unsourced. Dry snow mostly sheds off clothing and melts into it
/// slowly while it stays frozen. Wet snow near 0 C soaks more, which this one
/// number does not follow; the phase share moves part of the way there,
/// because inside the rain-snow band part of what falls is rain.
pub const SNOW_WETTING_SHARE: f32 = 1.0 / 3.0;

/// Swinbank's clear-sky constant (Swinbank 1963, Q. J. R. Meteorol. Soc. 89,
/// 339-348): a clear sky radiates like a black body at `0.0552 * T_air^1.5`
/// kelvin, about 20 K below the air on a mild night.
pub const SWINBANK_CLEAR_SKY: f64 = 0.0552;

/// Share of a standing person's view that is sky, in the open: about half,
/// the other half the ground (the six-direction weighting of ISO 7726 puts
/// the up and down directions on a standing body at a similar share of the
/// sides; half is the plain round figure).
pub const OPEN_SKY_VIEW: f64 = 0.5;

/// SolarCal (ASHRAE 55-2020, Appendix C), with the constants
/// pythermalcomfort's `solar_gain` uses: the radiative heat transfer
/// coefficient, W/m2 K.
const SOLARCAL_HR: f64 = 6.0;
/// Share of a standing body's surface that exchanges radiation.
const SOLARCAL_F_EFF: f64 = 0.725;
/// Short-wave (sunlight) absorptivity of skin and everyday clothing.
const SOLARCAL_ALPHA_SW: f64 = 0.7;
/// Long-wave absorptivity.
const SOLARCAL_ALPHA_LW: f64 = 0.95;
/// Diffuse sky light as a share of the clear-sky beam (SolarCal's default).
const DIFFUSE_SHARE: f64 = 0.2;
/// Ground reflectance outdoors: grass and bare soil reflect about a fifth of
/// the light (SolarCal's 0.6 is an indoor floor).
const GROUND_ALBEDO: f64 = 0.2;
/// The beam above the atmosphere in Meinel's clear-sky model, W/m2.
const MEINEL_BEAM_W_M2: f64 = 1353.0;

/// Direct-beam sunlight, W/m2 on a surface facing the sun, through a clear
/// sky with the sun at `sun_sin` (the sine of its height): Meinel and
/// Meinel (1976), `1353 * 0.7^(AM^0.678)`, with the air mass AM from Kasten
/// and Young (1989). About 950 W/m2 overhead and 430 at 10 degrees. 0 with
/// the sun down.
pub fn clear_sky_beam_w_m2(sun_sin: f32) -> f64 {
    let s = f64::from(sun_sin);
    if s <= 0.0 {
        return 0.0;
    }
    let elevation_deg = s.min(1.0).asin().to_degrees();
    let air_mass = 1.0 / (s + 0.50572 * (elevation_deg + 6.07995).powf(-1.6364));
    MEINEL_BEAM_W_M2 * 0.7_f64.powf(air_mass.powf(0.678))
}

/// The share of a standing body's radiating area the sun's beam falls on,
/// with the sun `elevation_deg` above the horizon, averaged over which way
/// the body faces (the player's facing is not tied to the sun):
/// `0.308 cos(b (0.998 - b^2 / 50000))`, b in degrees (Fanger 1970, the form
/// SOLWEIG uses). It is within 0.003 of pythermalcomfort's standing table
/// averaged over the azimuths at 0, 45, 60 and 90 degrees: about 0.31 with the
/// sun on the horizon and 0.08 with it overhead.
pub fn projected_area_factor(elevation_deg: f64) -> f64 {
    0.308 * (elevation_deg * (0.998 - elevation_deg * elevation_deg / 50_000.0)).to_radians().cos()
}

/// How far the sun raises a standing person's mean radiant temperature in
/// the open, C: SolarCal's effective radiant field of the diffuse sky light,
/// the direct beam and the light the ground reflects, turned into a rise of
/// the mean radiant temperature. Cloud takes the direct beam away (the
/// diffuse light is kept at the clear sky's, a simplification: an overcast
/// sky's diffuse light is of the same order). About 35 C with a clear sun
/// overhead, 23 C at 10 degrees and 14 C under an overcast noon.
pub fn sun_mrt_rise_c(sun_sin: f32, cloud: f32) -> f64 {
    let s = f64::from(sun_sin);
    if s <= 0.0 {
        return 0.0;
    }
    let clear_beam = clear_sky_beam_w_m2(sun_sin);
    let beam = clear_beam * (1.0 - f64::from(cloud.clamp(0.0, 1.0)));
    let diffuse = DIFFUSE_SHARE * clear_beam;
    let fp = projected_area_factor(s.min(1.0).asin().to_degrees());
    // In the open the whole sky vault is in view and the whole body in sun
    // (SolarCal's f_svv and f_bes are both 1).
    let e_diffuse = SOLARCAL_F_EFF * 0.5 * diffuse;
    let e_direct = SOLARCAL_F_EFF * fp * beam;
    let e_reflected = SOLARCAL_F_EFF * 0.5 * (beam * s + diffuse) * GROUND_ALBEDO;
    let erf = (e_diffuse + e_direct + e_reflected) * (SOLARCAL_ALPHA_SW / SOLARCAL_ALPHA_LW);
    erf / (SOLARCAL_HR * SOLARCAL_F_EFF)
}

/// The mean radiant temperature, C, a standing person in the open feels
/// in air at `air_c`, with `cloud` (0 clear to 1 overcast) of the sky under
/// cloud and the sun at `sun_sin` (the sine of its height; 0 or less while
/// it is down).
///
/// The long-wave part: half the view is ground, taken at the air's
/// temperature; half is sky, at Swinbank's clear-sky temperature where it is
/// clear and at the air's where cloud covers it (a cloud base radiates at
/// close to the air's temperature), mixed as fourth powers because radiation
/// goes as T^4. On a clear 10 C night that is about 0.5 C, nearly 10 degrees
/// below the air: the reason a clear night in the open feels so much colder
/// than the thermometer, and the reason a roof overhead (which radiates at
/// about the air's temperature) is warmer to sit under. The sun's part is
/// SolarCal's rise (`sun_mrt_rise_c`) on top, as ASHRAE 55 adds it: about
/// 47 C in all with a clear sun overhead in 20 C air, which is why shade
/// matters on a hot day.
pub fn open_sky_radiant_c(air_c: f32, cloud: f32, sun_sin: f32) -> f32 {
    let t_air = f64::from(air_c) + 273.15;
    let t_clear = SWINBANK_CLEAR_SKY * t_air.powf(1.5);
    let c = f64::from(cloud.clamp(0.0, 1.0));
    let sky4 = (1.0 - c) * t_clear.powi(4) + c * t_air.powi(4);
    let open4 = OPEN_SKY_VIEW * sky4 + (1.0 - OPEN_SKY_VIEW) * t_air.powi(4);
    (open4.powf(0.25) - 273.15 + sun_mrt_rise_c(sun_sin, cloud)) as f32
}

/// How hard rain or snow lands on an unsheltered person, 0 to 1, from what the
/// weather says falls there (`systems::precipitation`: the condition decides
/// how hard, the air where it falls decides rain or snow). Rain at full
/// weight, snow at `SNOW_WETTING_SHARE`.
pub fn precipitation(falling: Falling) -> f32 {
    (falling.rain + falling.snow * SNOW_WETTING_SHARE).clamp(0.0, 1.0)
}

/// Saturated water vapour pressure over water, mmHg (the model's own
/// Antoine-form fit, `p_sat_torr` in pythermalcomfort).
fn sat_vapour_mmhg(t_c: f64) -> f64 {
    (18.6686 - 4030.183 / (t_c + 235.0)).exp()
}

/// The air-side numbers every step needs.
struct Air {
    /// Convective heat transfer coefficient, W/m2/K.
    h_c: f64,
    /// Clothing area factor (clothed area over skin area).
    f_cl: f64,
    /// Lewis relation, K per mmHg (2.2 at sea level).
    lewis: f64,
    /// Vapour pressure of the air, mmHg.
    vapour_mmhg: f64,
    /// Most of the skin that can be wet with sweat.
    wet_max: f64,
    /// Clothing vapour permeation efficiency.
    i_cl: f64,
}

fn air(ex: &Exposure, clo: f64, met: f64) -> Air {
    // The model's pressure terms are in atmospheres. A near-vacuum is held at
    // 1 kPa so the numbers stay finite (the air supply kills first there).
    let atm = f64::from(ex.pressure_kpa).max(1.0) / 101.325;
    let v = ex.air_speed();
    // Natural convection, forced convection, and the convection the body's
    // own movement makes (ASHRAE 55's activity term, scaled by pressure here).
    let mut h_c = (3.0 * atm.powf(0.53)).max(8.600001 * (v * atm).powf(0.53));
    if met > 0.85 {
        h_c = h_c.max(5.66 * (met - 0.85).powf(0.39) * atm.powf(0.53));
    }
    Air {
        h_c,
        f_cl: 1.0 + 0.15 * clo,
        lewis: 2.2 / atm,
        vapour_mmhg: f64::from(ex.relative_humidity.clamp(0.0, 1.0)) * sat_vapour_mmhg(f64::from(ex.air_c)),
        wet_max: if clo > 0.0 { 0.59 * v.powf(-0.08) } else { 0.38 * v.powf(-0.29) },
        i_cl: if clo > 0.0 { 0.45 } else { 1.0 },
    }
}

/// The operative temperature, C (ASHRAE 55): the air's temperature and the
/// surroundings' radiant temperature, each weighted by how much heat the body
/// trades with it, convection with the air (`h_c`, the wind and the body's
/// own movement at `met`) and radiation with the surroundings (`h_r`,
/// linearised about their mean). What a person "feels" the air as: the
/// HUD's "feels" figure. Equal to the air's temperature where the
/// surroundings are at it (indoors, under a roof, overcast); well above it
/// standing in the sun in still air (about 35 C in 20 C air under a clear
/// noon sun), and below it under a clear night sky.
pub fn operative_c(ex: &Exposure, met: f32) -> f32 {
    let a = air(ex, 0.0, f64::from(met));
    let (t_air, t_rad) = (f64::from(ex.air_c), f64::from(ex.radiant_c));
    let h_r = 4.0 * EMISSIVITY * SIGMA * ((t_air + t_rad) / 2.0 + 273.15).powi(3) * RADIATING_SHARE;
    ((h_r * t_rad + a.h_c * t_air) / (h_r + a.h_c)) as f32
}

// -- State ---------------------------------------------------------------------------

/// One body's heat state. Held per person by the food system (it owns the
/// vitals); only the core temperature is saved (in `Vitals`), the rest
/// restarts from neutral on a load, which a real body does in minutes.
///
/// Kept in f64 on purpose: at 60 frames a second a cooling of a degree an
/// hour is 5e-6 C a frame, which is below an f32's resolution at 37 C, so an
/// f32 core would never move.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyHeat {
    /// Core temperature, C, as it really is (Realistic shows this).
    pub core_c: f64,
    /// Mean skin temperature, C.
    pub skin_c: f64,
    /// Skin blood flow, L per m2 per hour.
    blood_flow: f64,
    /// Share of the body's mass in the skin layer (Gagge's alpha; it grows
    /// as the vessels narrow, which is the body cooling its shell to keep the core).
    shell_share: f64,
    /// Heat leaving the skin as evaporating sweat and moisture, W/m2.
    skin_evaporation: f64,
    /// Shivering heat, W/m2.
    pub shiver_w_m2: f64,
    /// Clothing wetness, 0 (dry) to 1 (soaked).
    pub clothing_wetness: f64,
    /// Shivering done, in hours at the full peak (with the endurance exponent).
    shiver_load: f64,
    /// What is left of the shivering drive, 0 to 1.
    shiver_capacity: f64,
}

impl BodyHeat {
    /// A body at rest and comfortable, with its core at `core_c`.
    pub fn new(core_c: f64) -> Self {
        Self {
            core_c,
            skin_c: SKIN_NEUTRAL_C,
            blood_flow: BLOOD_FLOW_NEUTRAL,
            shell_share: 0.1,
            skin_evaporation: 0.0,
            shiver_w_m2: 0.0,
            clothing_wetness: 0.0,
            shiver_load: 0.0,
            shiver_capacity: 1.0,
        }
    }

    /// Is the body shivering hard enough to notice?
    pub fn is_shivering(&self) -> bool {
        self.shiver_w_m2 > SHIVERING_SHOWN_W_M2
    }

    /// Run the body for `dt_s` seconds wearing `clo` of clothing at `met` of
    /// activity. Long steps are cut into ten-second ones.
    pub fn step(&mut self, ex: &Exposure, clo: f32, met: f32, dt_s: f32) {
        let clo = f64::from(clo.max(0.0));
        let met = f64::from(met.max(0.0));
        let a = air(ex, clo, met);
        let mut left = f64::from(dt_s.max(0.0));
        while left > 0.0 {
            let dt = left.min(MAX_STEP_S);
            self.substep(ex, &a, clo, met, dt);
            left -= dt;
        }
    }

    fn substep(&mut self, ex: &Exposure, a: &Air, clo: f64, met: f64, dt: f64) {
        let t_air = f64::from(ex.air_c);
        let t_rad = f64::from(ex.radiant_c);
        // Clothing resistance, m2 K/W, less what wetness takes off it.
        let r_cl = 0.155 * clo * (1.0 - WET_INSULATION_LOSS * self.clothing_wetness);
        let metabolic = met * MET_W_M2 + self.shiver_w_m2;

        // The clothing surface temperature, iterated with the radiation
        // coefficient that depends on it (Gagge's loop, to 0.01 C).
        let mut t_cl = t_air;
        let (mut r_air, mut t_op) = (0.0, t_air);
        for _ in 0..30 {
            let h_r = 4.0 * EMISSIVITY * SIGMA * ((t_cl + t_rad) / 2.0 + 273.15).powi(3) * RADIATING_SHARE;
            let h_t = h_r + a.h_c;
            r_air = 1.0 / (a.f_cl * h_t);
            t_op = (h_r * t_rad + a.h_c * t_air) / h_t;
            let next = (r_air * self.skin_c + r_cl * t_op) / (r_air + r_cl);
            let done = (next - t_cl).abs() < 0.01;
            t_cl = next;
            if done {
                break;
            }
        }
        // Dry heat through the clothes; heat the blood carries core to skin;
        // breathing (latent and dry).
        let dry = (self.skin_c - t_op) / (r_air + r_cl);
        let core_to_skin = (self.core_c - self.skin_c) * (5.28 + 1.163 * self.blood_flow);
        let breath_latent = 0.0023 * metabolic * (44.0 - a.vapour_mmhg);
        let breath_dry = 0.0014 * metabolic * (34.0 - t_air);

        // Rain soaks the clothes; the water dries out of them, cooling the body.
        let r_evap_air = 1.0 / (a.lewis * a.f_cl * a.h_c);
        let wet_cooling = self.wet_clothes(ex, clo, t_cl, a, r_evap_air, dt);

        // Heat balance of each layer, and the temperatures it moves.
        let core_store = metabolic - core_to_skin - breath_latent - breath_dry;
        let skin_store = core_to_skin - dry - self.skin_evaporation - WET_CLOTHES_COOLING_SHARE * wet_cooling;
        let skin_capacity = TISSUE_HEAT_CAPACITY * self.shell_share * BODY_MASS_KG * 3600.0;
        let core_capacity = TISSUE_HEAT_CAPACITY * (1.0 - self.shell_share) * BODY_MASS_KG * 3600.0;
        self.skin_c += skin_store * BODY_AREA_M2 / skin_capacity * dt;
        self.core_c += core_store * BODY_AREA_M2 / core_capacity * dt;

        self.respond(a, r_cl, r_evap_air, dt);
    }

    /// Wetting by precipitation and drying by evaporation. Returns the latent
    /// heat leaving the clothes, W/m2 of skin.
    fn wet_clothes(&mut self, ex: &Exposure, clo: f64, t_cl: f64, a: &Air, r_evap_air: f64, dt: f64) -> f64 {
        let falling = if ex.sheltered { 0.0 } else { f64::from(ex.precipitation.clamp(0.0, 1.0)) };
        self.clothing_wetness = (self.clothing_wetness + falling / SOAK_S * dt).min(1.0);
        if self.clothing_wetness <= 0.0 || clo <= 0.0 {
            self.clothing_wetness = 0.0;
            return 0.0;
        }
        let evaporation = self.clothing_wetness * (sat_vapour_mmhg(t_cl) - a.vapour_mmhg).max(0.0) / r_evap_air;
        let water_kg = (WATER_KG_PER_CLO * clo).max(WATER_KG_MIN);
        let dried = evaporation * BODY_AREA_M2 / LATENT_HEAT_J_KG * dt / water_kg;
        self.clothing_wetness = (self.clothing_wetness - dried).max(0.0);
        evaporation
    }

    /// The body's responses for the next step: blood flow, sweat, shivering.
    fn respond(&mut self, a: &Air, r_cl: f64, r_evap_air: f64, dt: f64) {
        let skin_signal = self.skin_c - SKIN_NEUTRAL_C;
        let core_signal = self.core_c - CORE_NEUTRAL_C;
        let body_signal = self.shell_share * self.skin_c + (1.0 - self.shell_share) * self.core_c - BODY_NEUTRAL_C;
        let (skin_warm, skin_cold) = (skin_signal.max(0.0), (-skin_signal).max(0.0));
        let (core_warm, core_cold) = (core_signal.max(0.0), (-core_signal).max(0.0));

        self.blood_flow = ((BLOOD_FLOW_NEUTRAL + DILATE_GAIN * core_warm) / (1.0 + CONSTRICT_GAIN * skin_cold))
            .clamp(BLOOD_FLOW_MIN, BLOOD_FLOW_MAX);

        // Sweat, and how much of it the air can take (the skin's wettedness,
        // capped at the most that can be wet: sweat past it drips).
        let mut sweat_heat = 0.68 * (SWEAT_GAIN * body_signal.max(0.0) * (skin_warm / 10.7).exp()).min(SWEAT_MAX);
        let r_evap_clothes = r_cl / (a.lewis * a.i_cl);
        let evap_max = (sat_vapour_mmhg(self.skin_c) - a.vapour_mmhg) / (r_evap_air + r_evap_clothes);
        let diffusion;
        if evap_max <= 0.0 {
            sweat_heat = 0.0;
            diffusion = 0.0;
        } else {
            let wettedness = 0.06 + 0.94 * sweat_heat / evap_max;
            if wettedness > a.wet_max {
                let share = a.wet_max / 0.94;
                sweat_heat = share * evap_max;
                diffusion = 0.06 * (1.0 - share) * evap_max;
            } else {
                diffusion = wettedness * evap_max - sweat_heat;
            }
        }
        self.skin_evaporation = sweat_heat + diffusion;

        // Shivering: Gagge's drive, capped at what a person sustains, fading
        // out as the core passes 32 to 30 C, and scaled by what fatigue has left.
        let drive = (SHIVER_GAIN * skin_cold * core_cold).min(SHIVER_SUSTAINED_SHARE * SHIVER_PEAK_W_M2);
        let fade = ((self.core_c - SHIVER_STOPPED_C) / (SHIVER_STOPS_FROM_C - SHIVER_STOPPED_C)).clamp(0.0, 1.0);
        self.shiver_w_m2 = drive * fade * self.shiver_capacity;
        let dt_h = dt / 3600.0;
        if self.shiver_w_m2 > 0.05 * SHIVER_PEAK_W_M2 {
            self.shiver_load += (self.shiver_w_m2 / SHIVER_PEAK_W_M2).powf(SHIVER_ENDURANCE_EXPONENT) * dt_h;
            if self.shiver_load > SHIVER_ENDURANCE {
                self.shiver_capacity *= (1.0 - SHIVER_FATIGUE_PER_H).powf(dt_h);
            }
        } else {
            self.shiver_load = (self.shiver_load - SHIVER_RECOVERY_PER_H * dt_h).max(0.0);
            self.shiver_capacity = (self.shiver_capacity + SHIVER_RECOVERY_PER_H * dt_h).min(1.0);
        }

        // The skin layer's share of the body's mass follows the blood flow.
        self.shell_share = 0.0417737 + 0.7451833 / (self.blood_flow + 0.585417);
    }
}

// -- The vitals pass -----------------------------------------------------------------

/// One person's body heat as the food system keeps it between frames: the
/// model, and the core temperature it last showed (to notice when something
/// else set the core: a respawn, a loaded save).
#[derive(Debug, Clone, PartialEq)]
pub struct Tracked {
    pub body: BodyHeat,
    shown_c: f32,
}

impl Tracked {
    /// A body whose shown core is `shown_c` in `mode`.
    pub fn new(shown_c: f32, mode: Mode) -> Self {
        Self { body: BodyHeat::new(mode.real(f64::from(shown_c))), shown_c }
    }
}

/// What one frame of body heat did to a person: the health it cost (of a
/// 100-point body) and, when it cost any, the cause for the death screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatOutcome {
    pub harm: f32,
    pub cause: &'static str,
}

/// One frame of body heat for one person (the food system calls it for every
/// living body with vitals): run the model for `dt_s`, write the shown core
/// into `core_c`, set or clear the temperature conditions (refreshed to
/// `linger_s` while they hold), and return the harm.
#[allow(clippy::too_many_arguments)]
pub fn vitals_tick(
    tracked: &mut Tracked,
    core_c: &mut f32,
    effects: &mut crate::ecs::components::StatusEffects,
    ex: &Exposure,
    clo: f32,
    met: f32,
    mode: Mode,
    dt_s: f32,
    linger_s: f32,
) -> HeatOutcome {
    // Something else set the core (a respawn puts it back to 37 C, a load
    // restores the saved one): start the body over from it.
    if (*core_c - tracked.shown_c).abs() > 0.5 {
        *tracked = Tracked::new(*core_c, mode);
    }
    tracked.body.step(ex, clo, met, dt_s);
    let shown = mode.shown(tracked.body.core_c) as f32;
    *core_c = shown;
    tracked.shown_c = shown;

    for (id, on) in [
        ("hypothermia", shown < HYPOTHERMIA_C),
        ("heat_exhaustion", shown > HEAT_EXHAUSTION_C && shown <= HEAT_STROKE_C),
        ("heat_stroke", shown > HEAT_STROKE_C),
        ("shivering", tracked.body.is_shivering()),
    ] {
        if on {
            effects.apply(id, linger_s);
        } else {
            effects.remove(id);
        }
    }
    let harm = harm_per_s(shown, mode) * dt_s.max(0.0);
    let cause = if harm <= 0.0 {
        ""
    } else if shown < MODERATE_HYPOTHERMIA_C {
        "hypothermia"
    } else {
        "heatstroke"
    };
    HeatOutcome { harm, cause }
}

#[cfg(test)]
#[path = "body_heat_tests.rs"]
mod tests;

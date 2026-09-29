//! Weather system: dynamic weather simulation driven by season, randomness,
//! and (increment 4) the frame-locked body's environment.
//!
//! Stores `Weather` in the WeatherSystem struct. Other systems can read
//! weather state to affect farming, visibility, combat, etc.

use glam::Vec3;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::body_environment::{self, BodyEnvironment};
use crate::systems::env_layer1::{self, ClimateTable};
use crate::systems::time::{GameTime, Season};

/// Weather condition types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeatherCondition {
    Clear,
    Cloudy,
    Rain,
    Storm,
    Snow,
    Fog,
    Sandstorm,
}

/// Complete weather state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weather {
    /// Current weather condition: the weather SYSTEM (its clouds, its
    /// temperature deviation, whether it brings water and how hard). It does
    /// not decide rain versus snow: the air where the water falls does
    /// (`systems::precipitation`, `falling_at_player`, `condition_at_player`).
    pub condition: WeatherCondition,
    /// Weather intensity (0.0 = calm, 1.0 = extreme).
    pub intensity: f32,
    /// Wind speed in m/s.
    pub wind_speed: f32,
    /// Normalized wind direction vector.
    pub wind_direction: Vec3,
    /// Temperature in Celsius: the BODY-SURFACE GLOBAL reference for
    /// the whole frame-locked body (Earth = the calibrated seasonal
    /// table exactly; other bodies = catalog mean + the body-wide
    /// day/night swing). Body-scale simulations read THIS field:
    /// farming climate (systems/farming) and hydrology evaporation.
    /// It never carries a player-positional term (review split): the
    /// home station sits inside Earth's frame-lock envelope at ~400 km,
    /// and a positional altitude lapse here would have dragged the
    /// station's global climate arctic and wrongly punished farming
    /// aboard. Player-local temperature lives in `temperature_at_player`.
    pub temperature: f32,
    /// Temperature in Celsius AT THE PLAYER. On a world with a climate row
    /// (`data/environment/climate.ron`) it is environment LAYER 1's air
    /// temperature where the player stands (latitude, altitude, land or sea,
    /// the time of year: `systems::env_layer1`) plus the weather's DEVIATION
    /// (the condition's offset and its random spread, the same deviation the
    /// global field carries) plus the body-wide day/night swing. Elsewhere it
    /// is `temperature` plus the generic body model's latitude and altitude
    /// delta. Walking up a mountain cools this one; the global field never
    /// moves. Player-body consumers read THIS field: the survival exposure
    /// path (engine::survival_env, the body heat model) and the HUD
    /// thermometer. Recomputed at export time each tick; WeatherSystem does
    /// not simulate it internally because the positional part must not fight
    /// the 30 s transition lerps.
    #[serde(default)]
    pub temperature_at_player: f32,
    /// Air pressure AT THE PLAYER, kPa: Layer 1's column at the player's
    /// altitude on a world with a climate row, Earth's standard column on a
    /// world with air but no row, and 0 in open space or on an airless body.
    /// The body heat model reads it (convection and sweat both depend on it).
    #[serde(default)]
    pub pressure_kpa_at_player: f32,
    /// Wind AT THE PLAYER toward the east and toward the north, m/s: Layer 1's
    /// prevailing wind (the trades, the westerlies) plus the weather's own
    /// wind. While the F11 panel drives the weather, only the panel's wind:
    /// "calm" has to mean calm there. See `wind_speed_at_player`.
    #[serde(default)]
    pub wind_east_at_player: f32,
    #[serde(default)]
    pub wind_north_at_player: f32,
    /// Relative humidity (0.0-1.0).
    pub humidity: f32,
    /// Visibility factor (0.0 = blind, 1.0 = clear).
    pub visibility: f32,
    /// Seconds remaining in the current transition (0 = fully transitioned).
    pub transition_timer: f32,
    /// Active extreme-weather event (v0.1035, data/weather/events.ron):
    /// id + display name, empty when none, and seconds left. The HUD
    /// shows the name; the precipitation block plays its emitters.
    #[serde(default)]
    pub event_id: String,
    #[serde(default)]
    pub event_name: String,
    #[serde(default)]
    pub event_remaining_s: f32,
}

impl Weather {
    /// Wind speed at the player, m/s (the magnitude of the east and north
    /// components). What the body heat model is exposed to.
    pub fn wind_speed_at_player(&self) -> f32 {
        self.wind_east_at_player.hypot(self.wind_north_at_player)
    }

    /// The sea state the wind where the player is raises, 0 (glassy) to 1
    /// (a full storm sea): 2 m/s or less reads glassy, about 15 m/s a storm
    /// (v0.909's mapping). It reads the wind AT THE PLAYER, environment Layer
    /// 1's prevailing wind with the weather's deviation on top, not the
    /// weather's global wind (2026-09-28, PRIORITIES item 1): the sea the
    /// player looks at is the sea where they are.
    pub fn sea_state_target(&self) -> f32 {
        ((self.wind_speed_at_player() - 2.0) / 13.0).clamp(0.0, 1.0)
    }
}

/// The wind the FFT sea's spectrum is built for, m/s: a showcase sea pin
/// mapped onto 0.5 to 25 m/s, else the wind AT THE PLAYER (east and north
/// components, as the GUI's weather mirror carries them), else 8 m/s before
/// any weather exists. The wave shapes and the sea state's shading
/// (`Weather::sea_state_target`) must read the same wind: a calm spot under
/// a global storm drew storm waves with calm-sea shading when this read the
/// global wind (2026-09-28).
pub fn ocean_fft_wind_target(pin: Option<f32>, wind_at_player: Option<(f32, f32)>) -> f32 {
    match (pin, wind_at_player) {
        (Some(p), _) => 0.5 + p.clamp(0.0, 1.0) * 24.5,
        (None, Some((east, north))) => east.hypot(north),
        (None, None) => 8.0,
    }
}

impl Default for Weather {
    fn default() -> Self {
        Self {
            condition: WeatherCondition::Clear,
            intensity: 0.0,
            wind_speed: 2.0,
            wind_direction: Vec3::new(1.0, 0.0, 0.0).normalize(),
            temperature: 20.0,
            // Matches `temperature` at rest: with no positional delta
            // published yet, the player's local reading IS the global one.
            temperature_at_player: 20.0,
            // The default body environment is open space (the home station),
            // so no air outside the hull until a tick says otherwise.
            pressure_kpa_at_player: 0.0,
            // The weather's own wind, as the export would read it.
            wind_east_at_player: 2.0,
            wind_north_at_player: 0.0,
            humidity: 0.4,
            visibility: 1.0,
            transition_timer: 0.0,
            event_id: String::new(),
            event_name: String::new(),
            event_remaining_s: 0.0,
        }
    }
}

/// LIVE WEATHER CONTROL (v0.1050, operator: "is there a way for me to cycle
/// the weather live in game? Like press F11 to bring up the weather menu").
/// Published into the DataStore under "weather_control" exactly like
/// `time_set_hour_request`, because WeatherSystem lives inside the
/// SystemRunner and is not otherwise reachable from the GUI. While `manual`
/// is Some the random condition roll and the extreme-event roll are both
/// suspended, so a chosen sky STAYS chosen.
///
/// This is permanent dev tooling, not a debug hack: the ocean's whole
/// character is wind-driven (JONSWAP fetch), so being able to drive wind and
/// condition live is the only practical way to see - or review - calm glass
/// through to a storm sea.
#[derive(Debug, Clone, Default)]
pub struct WeatherControl {
    pub manual: Option<ManualWeather>,
    /// Set by the panel when the operator picks a condition; consumed by the
    /// next tick so the ancillary values (visibility, temperature, humidity)
    /// ramp through the SAME 30 s transition the sim uses.
    pub retrigger: bool,
}

/// The values the panel drives directly.
#[derive(Debug, Clone, Copy)]
pub struct ManualWeather {
    pub condition: WeatherCondition,
    pub intensity: f32,
    pub wind_speed: f32,
}

// The weather's timers run on the one game clock (2026-09-27): game seconds,
// so a front, a storm and the gap between them take the same game hours at
// any time speed. The roll cadence keeps the share of a day it had on the
// 20-minute day (5 to 15 minutes of it were 6 to 18 game hours).

/// Game seconds a natural change of weather takes to arrive: half an hour,
/// the pace of a front passing.
const TRANSITION_DURATION: f32 = 1800.0;
/// Seconds a change the player asked for (the F11 panel) or an arrival on a
/// new world takes: half a minute, on whichever of real or game time is
/// faster, so it still arrives with the clock held still.
const QUICK_TRANSITION_S: f32 = 30.0;

/// Minimum game seconds between weather changes (6 hours).
const MIN_CHANGE_INTERVAL: f32 = 6.0 * 3600.0;

/// Maximum game seconds between weather changes (18 hours).
const MAX_CHANGE_INTERVAL: f32 = 18.0 * 3600.0;

/// Game seconds between extreme-event roll attempts (v0.1035; 18 hours, the
/// same share of a day as the 15 minutes it was on the 20-minute day).
const EVENT_ROLL_INTERVAL_S: f32 = 18.0 * 3600.0;

/// Chance per roll that an eligible extreme event actually fires - the
/// registry's rarity weights then decide WHICH one.
const EVENT_FIRE_CHANCE: f32 = 0.35;

/// Fraction of a Front profile's gust speed added to the exported wind
/// while its event runs (steady component; real gust pulsing comes with
/// the wind-field rung).
const EVENT_GUST_EXPORT: f32 = 0.6;

/// Drives weather transitions based on season and random rolls.
pub struct WeatherSystem {
    weather: Weather,
    /// Previous weather values for lerping during transitions.
    prev_intensity: f32,
    prev_visibility: f32,
    prev_temp_dev: f32,
    prev_humidity: f32,
    prev_wind_speed: f32,
    /// Target values for the new condition.
    target_intensity: f32,
    target_visibility: f32,
    target_temp_dev: f32,
    target_humidity: f32,
    target_wind_speed: f32,
    /// The weather's temperature DEVIATION, C: the condition's offset (clear
    /// +3, rain -3, a storm -5, snow forced to freezing at the reference) plus
    /// its random spread, ramped through transitions. It is the ONE thing the
    /// weather adds to temperature; the base under it is the body's reference
    /// climate for the global field (farming and hydrology read that) and
    /// environment Layer 1 at the player (the body heat model reads that).
    /// Kept as a deviation rather than an absolute so the base can move under
    /// it (a new season, a walk up a mountain) without waiting for the next
    /// condition change.
    temp_dev: f32,
    /// Countdown until the next weather change attempt.
    next_change_timer: f32,
    /// How long the transition in progress takes (TRANSITION_DURATION or
    /// QUICK_TRANSITION_S).
    transition_len: f32,
    /// Countdown until the next extreme-event roll (v0.1035).
    event_roll_timer: f32,
    /// Steady wind bonus (m/s) exported while a Front-profile event is
    /// active; 0 otherwise. Kept OUT of the lerp targets so event wind
    /// vanishes cleanly the moment the event ends.
    active_gust_mps: f32,
    /// The body whose weather is being simulated (artificial-planet
    /// increment 4): published each frame by the main loop from the
    /// frame-locked body. Its default is the Earth home frame, so the
    /// pre-increment behavior (Earth weather at the home station) is
    /// exactly preserved when nothing publishes the snapshot.
    env: BodyEnvironment,
    /// Random number generator (Send + Sync compatible).
    rng: StdRng,
}

impl WeatherSystem {
    pub fn new() -> Self {
        let weather = Weather::default();
        // The default weather's 20 C against the default body's Spring
        // reference: the deviation that reproduces it.
        let temp_dev = weather.temperature
            - body_environment::body_baseline_temp_c(&BodyEnvironment::default(), Season::Spring);
        Self {
            prev_intensity: weather.intensity,
            prev_visibility: weather.visibility,
            prev_temp_dev: temp_dev,
            prev_humidity: weather.humidity,
            prev_wind_speed: weather.wind_speed,
            target_intensity: weather.intensity,
            target_visibility: weather.visibility,
            target_temp_dev: temp_dev,
            target_humidity: weather.humidity,
            target_wind_speed: weather.wind_speed,
            temp_dev,
            weather,
            next_change_timer: 60.0, // First change after 1 minute
            transition_len: TRANSITION_DURATION,
            event_roll_timer: EVENT_ROLL_INTERVAL_S,
            active_gust_mps: 0.0,
            env: BodyEnvironment::default(),
            rng: StdRng::from_os_rng(),
        }
    }

    /// Get current weather state (for systems that need to read it directly).
    pub fn weather(&self) -> &Weather {
        &self.weather
    }

    /// Pick a new weather condition based on the current season.
    fn pick_condition(&mut self, season: Season) -> WeatherCondition {
        let roll: f32 = self.rng.gen();
        match season {
            Season::Spring => {
                // Mostly clear/cloudy, occasional rain
                if roll < 0.35 {
                    WeatherCondition::Clear
                } else if roll < 0.65 {
                    WeatherCondition::Cloudy
                } else if roll < 0.90 {
                    WeatherCondition::Rain
                } else if roll < 0.95 {
                    WeatherCondition::Fog
                } else {
                    WeatherCondition::Storm
                }
            }
            Season::Summer => {
                // Clear with rare storms
                if roll < 0.55 {
                    WeatherCondition::Clear
                } else if roll < 0.80 {
                    WeatherCondition::Cloudy
                } else if roll < 0.90 {
                    WeatherCondition::Rain
                } else if roll < 0.95 {
                    WeatherCondition::Sandstorm
                } else {
                    WeatherCondition::Storm
                }
            }
            Season::Autumn => {
                // Cloudy/rain, occasional fog
                if roll < 0.20 {
                    WeatherCondition::Clear
                } else if roll < 0.45 {
                    WeatherCondition::Cloudy
                } else if roll < 0.75 {
                    WeatherCondition::Rain
                } else if roll < 0.90 {
                    WeatherCondition::Fog
                } else {
                    WeatherCondition::Storm
                }
            }
            Season::Winter => {
                // Snow, fog, cloudy
                if roll < 0.10 {
                    WeatherCondition::Clear
                } else if roll < 0.35 {
                    WeatherCondition::Cloudy
                } else if roll < 0.65 {
                    WeatherCondition::Snow
                } else if roll < 0.85 {
                    WeatherCondition::Fog
                } else if roll < 0.95 {
                    WeatherCondition::Rain
                } else {
                    WeatherCondition::Storm
                }
            }
        }
    }

    /// Compute target weather parameters for a given condition and season.
    fn compute_targets(&mut self, condition: WeatherCondition, season: Season) {
        // The temperature target is a DEVIATION (see `temp_dev`). The base it
        // rides on is applied every tick: the body's reference climate for
        // the global field (Earth: the calibrated seasonal table; other
        // bodies: the catalog mean), environment Layer 1 at the player. Only
        // snow needs the base here, to keep its "must be freezing" meaning at
        // the reference point where the old absolute rule applied.
        let base_temp = body_environment::body_baseline_temp_c(&self.env, season);

        // Add some random variance to temperature (+/- 5 degrees)
        let temp_variance: f32 = self.rng.gen_range(-5.0..5.0);

        match condition {
            WeatherCondition::Clear => {
                self.target_intensity = 0.0;
                self.target_visibility = 1.0;
                self.target_temp_dev = temp_variance + 3.0; // Clear = slightly warmer
                self.target_humidity = 0.3 + self.rng.gen_range(0.0..0.1);
                self.target_wind_speed = self.rng.gen_range(0.5..3.0);
            }
            WeatherCondition::Cloudy => {
                self.target_intensity = self.rng.gen_range(0.2..0.5);
                self.target_visibility = 0.8;
                self.target_temp_dev = temp_variance;
                self.target_humidity = 0.5 + self.rng.gen_range(0.0..0.2);
                self.target_wind_speed = self.rng.gen_range(2.0..6.0);
            }
            WeatherCondition::Rain => {
                self.target_intensity = self.rng.gen_range(0.4..0.8);
                self.target_visibility = 0.6;
                self.target_temp_dev = temp_variance - 3.0; // Rain cools
                self.target_humidity = 0.8 + self.rng.gen_range(0.0..0.2);
                self.target_wind_speed = self.rng.gen_range(3.0..8.0);
            }
            WeatherCondition::Storm => {
                self.target_intensity = self.rng.gen_range(0.8..1.0);
                self.target_visibility = 0.4;
                self.target_temp_dev = temp_variance - 5.0;
                self.target_humidity = 0.9 + self.rng.gen_range(0.0..0.1);
                self.target_wind_speed = self.rng.gen_range(10.0..20.0);
            }
            WeatherCondition::Snow => {
                self.target_intensity = self.rng.gen_range(0.3..0.7);
                self.target_visibility = 0.5;
                // Must be freezing at the reference: the deviation that puts
                // the reference at or below 0 C. That is the Snow SYSTEM's cold
                // snap; whether what falls is rain or snow comes from the air
                // where it falls (systems::precipitation), so a Snow roll over
                // the tropics still rains there.
                self.target_temp_dev = (base_temp + temp_variance).min(0.0) - base_temp;
                self.target_humidity = 0.7 + self.rng.gen_range(0.0..0.2);
                self.target_wind_speed = self.rng.gen_range(2.0..7.0);
            }
            WeatherCondition::Fog => {
                self.target_intensity = self.rng.gen_range(0.5..0.9);
                self.target_visibility = 0.2;
                self.target_temp_dev = temp_variance - 1.0;
                self.target_humidity = 0.9 + self.rng.gen_range(0.0..0.1);
                self.target_wind_speed = self.rng.gen_range(0.0..2.0);
            }
            WeatherCondition::Sandstorm => {
                self.target_intensity = self.rng.gen_range(0.6..1.0);
                self.target_visibility = 0.3;
                self.target_temp_dev = temp_variance + 5.0; // Hot
                self.target_humidity = 0.1 + self.rng.gen_range(0.0..0.1);
                self.target_wind_speed = self.rng.gen_range(12.0..25.0);
            }
        }

        // Body caps (increment 4): the targets above were written for an
        // Earth-like sky. No atmosphere means no wind, no haze, and no
        // moisture at all (the Moon's "weather" is only its brutal
        // temperature); an atmosphere without surface water carries
        // almost no humidity (Mars).
        if !self.env.has_atmosphere {
            self.target_intensity = 0.0;
            self.target_visibility = 1.0;
            self.target_humidity = 0.0;
            self.target_wind_speed = 0.0;
        } else if !self.env.has_water {
            self.target_humidity = self.target_humidity.min(0.1);
        }
    }

    /// Start a transition to a new weather condition.
    fn begin_transition(&mut self, new_condition: WeatherCondition, season: Season, len: f32) {
        // Snapshot current values for lerping
        self.prev_intensity = self.weather.intensity;
        self.prev_visibility = self.weather.visibility;
        self.prev_temp_dev = self.temp_dev;
        self.prev_humidity = self.weather.humidity;
        self.prev_wind_speed = self.weather.wind_speed;

        self.weather.condition = new_condition;
        self.weather.transition_timer = len;
        self.transition_len = len.max(1e-3);
        self.compute_targets(new_condition, season);

        // Randomize wind direction on weather change
        let angle: f32 = self.rng.gen_range(0.0..std::f32::consts::TAU);
        self.weather.wind_direction = Vec3::new(angle.cos(), 0.0, angle.sin()).normalize();
    }
}

impl System for WeatherSystem {
    fn name(&self) -> &str {
        "WeatherSystem"
    }

    fn tick(&mut self, _world: &mut hecs::World, dt: f32, data: &DataStore) {
        // Determine current season + hour from the GameTime that TimeSystem
        // exports into the DataStore (behind a Mutex); fall back to Spring
        // noon if absent. The hour feeds the day/night temperature swing on
        // airless bodies (increment 4).
        // The year fraction is Layer 1's seasonal clock (a smooth annual
        // cycle; `season` stays the four-step label the rolls use). Its
        // fallback is the same moment: noon of day 0.
        let (season, hour, year_fraction) = data
            .get::<std::sync::Mutex<GameTime>>("game_time")
            .and_then(|m| m.lock().ok())
            .map(|gt| (gt.season, gt.solar_hour(), gt.year_fraction()))
            .unwrap_or((
                Season::Spring,
                12.0,
                0.5 / f64::from(crate::systems::time::DEFAULT_DAYS_PER_YEAR),
            ));

        // Which body's weather are we simulating? (increment 4) The main
        // loop publishes the frame-locked body's snapshot each frame;
        // absent (tests, headless, pre-first-frame) means the Earth home
        // default, i.e. the pre-increment behavior.
        let new_env = data
            .get::<BodyEnvironment>("body_environment")
            .cloned()
            .unwrap_or_default();
        let body_changed = new_env.body_id != self.env.body_id
            || new_env.has_atmosphere != self.env.has_atmosphere
            || new_env.has_water != self.env.has_water;
        self.env = new_env;
        if body_changed {
            // Arriving at a different world retunes the sky immediately.
            // The normal roll cadence is 5 to 15 minutes, far too slow for
            // an FTL hop from Earth rain to lunar vacuum; begin_transition
            // re-runs compute_targets against the NEW body's baseline even
            // when the condition name stays the same, so temperature and
            // wind ramp over the normal 30 s instead of waiting.
            let cond = body_environment::sanitize_condition(self.weather.condition, &self.env);
            self.begin_transition(cond, season, QUICK_TRANSITION_S);
            // A running extreme event does not follow you to a world that
            // cannot host it (a thunderstorm has no business on the Moon).
            if !(self.env.has_atmosphere && self.env.has_water)
                && self.weather.event_remaining_s > 0.0
            {
                log::info!(
                    "[WeatherEvent] '{}' dropped: body change to {}",
                    self.weather.event_name,
                    self.env.body_id
                );
                self.weather.event_remaining_s = 0.0;
                self.weather.event_id.clear();
                self.weather.event_name.clear();
                self.active_gust_mps = 0.0;
            }
        }

        // Live control from the F11 panel (v0.1050). Read first: it decides
        // whether the random rolls below run at all.
        let manual = data
            .get::<std::sync::Mutex<WeatherControl>>("weather_control")
            .and_then(|m| m.lock().ok())
            .and_then(|mut c| {
                let retrigger = c.retrigger;
                c.retrigger = false;
                c.manual.map(|m| (m, retrigger))
            });
        if let Some((m, retrigger)) = manual {
            // A condition change goes through begin_transition so visibility,
            // temperature and humidity ramp naturally rather than snapping;
            // wind and intensity are then held at the panel's values below.
            if retrigger || m.condition != self.weather.condition {
                self.begin_transition(m.condition, season, QUICK_TRANSITION_S);
            }
        }

        // Game seconds this tick (the one clock); dt stays real seconds.
        let game_dt = crate::systems::time::scaled_dt(dt, data);
        // Count down to next weather change
        self.next_change_timer -= game_dt;
        if manual.is_none() && self.next_change_timer <= 0.0 {
            // The roll still uses the Earth-tuned season odds; sanitize
            // clamps the result to what THIS body can host (increment 4):
            // airless worlds always come back Clear, dry atmospheres never
            // rain/snow/fog and storm as dust storms instead.
            let new_condition = body_environment::sanitize_condition(
                self.pick_condition(season),
                &self.env,
            );
            if new_condition != self.weather.condition {
                self.begin_transition(new_condition, season, TRANSITION_DURATION);
            }
            // Schedule next change
            self.next_change_timer = self.rng.gen_range(MIN_CHANGE_INTERVAL..MAX_CHANGE_INTERVAL);
        }

        // Process smooth transition
        if self.weather.transition_timer > 0.0 {
            self.weather.transition_timer = (self.weather.transition_timer - dt.max(game_dt)).max(0.0);
            let t = (1.0 - self.weather.transition_timer / self.transition_len).clamp(0.0, 1.0);
            // Smooth-step for more natural transitions
            let t = t * t * (3.0 - 2.0 * t);

            self.weather.intensity = lerp(self.prev_intensity, self.target_intensity, t);
            self.weather.visibility = lerp(self.prev_visibility, self.target_visibility, t);
            self.temp_dev = lerp(self.prev_temp_dev, self.target_temp_dev, t);
            self.weather.humidity = lerp(self.prev_humidity, self.target_humidity, t);
            self.weather.wind_speed = lerp(self.prev_wind_speed, self.target_wind_speed, t);
        }
        // The global reference follows its base every tick (a season turning
        // over reaches farming now, not at the next condition change) with the
        // weather's deviation on top. Set BEFORE the event roll below, which
        // reads it.
        self.weather.temperature = body_environment::body_baseline_temp_c(&self.env, season) + self.temp_dev;

        // The panel's wind + intensity win over the lerp, so dragging a slider
        // is immediate instead of being walked back over 30 s.
        if let Some((m, _)) = manual {
            self.weather.intensity = m.intensity;
            self.weather.wind_speed = m.wind_speed;
            self.target_intensity = m.intensity;
            self.target_wind_speed = m.wind_speed;
        }

        // ── Extreme-weather events (v0.1035, data/weather/events.ron) ──
        // A running event counts down; otherwise roll periodically for an
        // eligible one. Selection is season/temp/wind-gated + rarity-
        // weighted (systems::weather_events). This rung applies Front
        // gusts to the exported wind and surfaces the event to the HUD +
        // precipitation; Vortex spatial wind and hazard damage are the
        // NEXT rung (logged on activation so playtests can spot them).
        if self.weather.event_remaining_s > 0.0 {
            self.weather.event_remaining_s = (self.weather.event_remaining_s - game_dt).max(0.0);
            if self.weather.event_remaining_s == 0.0 {
                log::info!("[WeatherEvent] '{}' ended", self.weather.event_name);
                self.weather.event_id.clear();
                self.weather.event_name.clear();
                self.active_gust_mps = 0.0;
            }
        } else if manual.is_none() && self.env.has_atmosphere && self.env.has_water {
            // No random extreme events while the F11 panel is driving, and
            // none at all on bodies that cannot host them (increment 4):
            // the shipped registry is Earth-authored (thunderstorms,
            // tornadoes); per-body event profiles (Mars dust fronts) are a
            // later increment per docs/design/artificial-planet.md.
            self.event_roll_timer -= game_dt;
            if self.event_roll_timer <= 0.0 {
                self.event_roll_timer = EVENT_ROLL_INTERVAL_S;
                if self.rng.gen::<f32>() < EVENT_FIRE_CHANCE {
                    if let Some(reg) = data
                        .get::<crate::systems::weather_events::WeatherEventRegistry>(
                            "weather_event_registry",
                        )
                    {
                        let season_name = format!("{season:?}");
                        let elig = crate::systems::weather_events::eligible(
                            reg,
                            &season_name,
                            self.weather.temperature,
                            self.weather.wind_speed,
                        );
                        let roll: f32 = self.rng.gen();
                        if let Some(ev) =
                            crate::systems::weather_events::weighted_pick(&elig, roll)
                        {
                            self.weather.event_id = ev.id.clone();
                            self.weather.event_name = ev.name.clone();
                            self.weather.event_remaining_s = self
                                .rng
                                .gen_range(ev.duration_s.min..=ev.duration_s.max);
                            use crate::systems::weather_events::WindProfile as WP;
                            self.active_gust_mps = match ev.wind_profile {
                                WP::Front { gust_mps, .. } => gust_mps,
                                WP::Vortex { peak_mps, .. } => {
                                    log::info!(
                                        "[WeatherEvent] vortex wind ({peak_mps} m/s core) + hazard deferred to the next rung"
                                    );
                                    0.0
                                }
                                WP::None => 0.0,
                            };
                            log::info!(
                                "[WeatherEvent] '{}' started ({:.0}s)",
                                self.weather.event_name,
                                self.weather.event_remaining_s
                            );
                        }
                    }
                }
            }
        }

        // Export the current weather to the DataStore. Interior mutability
        // via a Mutex (the TimeSystem/game_time pattern) since tick only gets
        // &DataStore. Front-event gusts ride the EXPORT only, so the internal
        // lerp targets stay clean and event wind vanishes with the event.
        //
        // TEMPERATURE SPLITS IN TWO AT EXPORT (review fix; see the Weather
        // struct field docs):
        //  - `temperature` is the BODY-SURFACE GLOBAL reference: the ramped
        //    baseline (Earth's calibrated table / another body's catalog
        //    mean) + condition offsets + the body-wide day/night swing (one
        //    game clock, so the swing is global in this v1 model; zero on
        //    Earth, whose table already carries its day cycle). Farming
        //    climate and hydrology evaporation read this. It must NEVER
        //    carry a player-positional term: the 400 km home station lives
        //    inside Earth's frame-lock envelope, and the altitude lapse
        //    would have turned its global climate arctic and punished
        //    farming aboard.
        //  - the AT-PLAYER values (temperature, pressure, wind) come from
        //    environment Layer 1 where the player stands, with the weather's
        //    deviation on top: `air_at_player` below. The survival exposure
        //    path (the body heat model) and the HUD thermometer read these.
        //    Riding the export only means they track the player instantly
        //    while the internal deviation keeps ramping through transitions;
        //    the default Earth home environment (not on any world) reads the
        //    global temperature exactly.
        if let Some(slot) = data.get::<std::sync::Mutex<Weather>>("weather") {
            if let Ok(mut w) = slot.lock() {
                *w = self.weather.clone();
                w.temperature = self.weather.temperature
                    + body_environment::diurnal_swing_c(&self.env, hour);
                w.wind_speed += self.active_gust_mps * EVENT_GUST_EXPORT;
                // The store's table (disk first, a modder's edit wins), else
                // the copy compiled into the binary. (A closure, not the bare
                // fn path: the path would pin the Option to 'static.)
                let table = data
                    .get::<ClimateTable>(env_layer1::STORE_KEY)
                    .or_else(|| ClimateTable::shipped());
                let at = air_at_player(&AtPlayerInputs {
                    env: &self.env,
                    table,
                    season,
                    hour,
                    year_fraction,
                    global_c: w.temperature,
                    temp_dev_c: self.temp_dev,
                    // The weather's own wind has no geographic frame yet (its
                    // direction is rolled at random), so its rolled direction
                    // is read as (east, north).
                    weather_wind: (w.wind_direction.x * w.wind_speed, w.wind_direction.z * w.wind_speed),
                    manual_wind: manual.is_some(),
                });
                w.temperature_at_player = at.temp_c;
                w.pressure_kpa_at_player = at.pressure_kpa;
                w.wind_east_at_player = at.wind_east;
                w.wind_north_at_player = at.wind_north;
            }
        }
    }
}

/// Sea-level temperature, K, of the column used for a world with air but no
/// climate row: the 1976 US Standard Atmosphere's 15 C.
const STANDARD_SEA_LEVEL_K: f64 = 288.15;

/// What `air_at_player` needs: where the player is, the climate data, the
/// clock, and what the weather itself is doing.
pub struct AtPlayerInputs<'a> {
    pub env: &'a BodyEnvironment,
    pub table: Option<&'a ClimateTable>,
    pub season: Season,
    pub hour: f32,
    pub year_fraction: f64,
    /// The exported global reference, C (base + deviation + day/night swing).
    pub global_c: f32,
    /// The weather's temperature deviation, C.
    pub temp_dev_c: f32,
    /// The weather's own wind (east, north), m/s, gusts included.
    pub weather_wind: (f32, f32),
    /// The F11 panel is driving: its wind is the whole wind.
    pub manual_wind: bool,
}

/// The air AT THE PLAYER, as the weather exports it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtPlayer {
    pub temp_c: f32,
    pub pressure_kpa: f32,
    pub wind_east: f32,
    pub wind_north: f32,
}

/// Environment Layer 1 where the player stands, with the weather's deviation
/// on top (docs/design/environment-fields.md, "Layer 1 as built"). Pure, so
/// the equator, the mountain and the pole can be tested without a DataStore.
///
/// - Standing on a world WITH a climate row: Layer 1's air temperature at the
///   player's direction, altitude, land share and date, plus the deviation and
///   the body-wide day/night swing; Layer 1's pressure; Layer 1's prevailing
///   wind plus the weather's own. No air means no pressure and no wind.
/// - Anywhere else (open space, the home station, a world with no row): the
///   global reference plus the generic body model's latitude and altitude
///   delta, as before Layer 1; Earth's standard column for pressure on a
///   world with air, 0 without; the weather's wind alone.
pub fn air_at_player(i: &AtPlayerInputs) -> AtPlayer {
    let env = i.env;
    let row = if env.locked { i.table.and_then(|t| t.row(&env.body_id)) } else { None };
    let (temp_c, pressure_kpa, prevailing) = match row {
        Some(row) => {
            let air = row.air_at(
                env.up_dir,
                f64::from(env.altitude_m),
                f64::from(env.land_fraction),
                i.year_fraction,
            );
            let temp = air.temp_c as f32 + i.temp_dev_c + body_environment::diurnal_swing_c(env, i.hour);
            if env.has_atmosphere {
                (temp, air.pressure_kpa as f32, (air.wind_east as f32, air.wind_north as f32))
            } else {
                (temp, 0.0, (0.0, 0.0))
            }
        }
        None => {
            let temp = i.global_c + body_environment::positional_temp_offset_c(env, i.season, i.hour);
            let pressure = if env.locked && env.has_atmosphere {
                i.table
                    .and_then(|t| t.row("earth"))
                    .map(|e| e.column(STANDARD_SEA_LEVEL_K, f64::from(env.altitude_m)).1 as f32)
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            (temp, pressure, (0.0, 0.0))
        }
    };
    let (we, wn) = i.weather_wind;
    let (wind_east, wind_north) = if i.manual_wind { (we, wn) } else { (prevailing.0 + we, prevailing.1 + wn) };
    AtPlayer { temp_c, pressure_kpa, wind_east, wind_north }
}

/// Linear interpolation.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod sea_state_tests {
    use super::*;

    /// A storm in the global weather but calm air where the player stands
    /// gives a calm sea, and the reverse a rough one. Red check, run: making
    /// `sea_state_target` read `wind_speed` (what the frame loop did before)
    /// fails the first assertion.
    #[test]
    fn the_sea_follows_the_wind_where_the_player_is() {
        // The spectrum's wind reads the same at-player wind.
        assert_eq!(ocean_fft_wind_target(None, Some((1.2, 0.9))), 1.5);
        assert_eq!(ocean_fft_wind_target(Some(1.0), Some((1.2, 0.9))), 25.0, "a pin wins");
        assert_eq!(ocean_fft_wind_target(None, None), 8.0);
        let mut w = Weather { wind_speed: 15.0, ..Weather::default() };
        w.wind_east_at_player = 1.2;
        w.wind_north_at_player = 0.9;
        assert_eq!(w.sea_state_target(), 0.0, "1.5 m/s here: glassy, whatever the global wind");
        w.wind_speed = 1.0;
        w.wind_east_at_player = 12.0;
        w.wind_north_at_player = 9.0;
        assert_eq!(w.sea_state_target(), 1.0, "15 m/s here: a storm sea");
        w.wind_east_at_player = 8.5;
        w.wind_north_at_player = 0.0;
        assert!((w.sea_state_target() - 0.5).abs() < 1e-6, "8.5 m/s: halfway");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_weather() {
        let w = Weather::default();
        assert_eq!(w.condition, WeatherCondition::Clear);
        assert!((w.visibility - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_lerp() {
        assert!((lerp(0.0, 10.0, 0.5) - 5.0).abs() < f32::EPSILON);
        assert!((lerp(0.0, 10.0, 0.0) - 0.0).abs() < f32::EPSILON);
        assert!((lerp(0.0, 10.0, 1.0) - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_weather_system_ticks() {
        let mut system = WeatherSystem::new();
        let mut world = hecs::World::new();
        let data = DataStore::new();

        // Tick a few times; should not panic
        for _ in 0..100 {
            system.tick(&mut world, 1.0 / 60.0, &data);
        }
    }

    /// v0.1035: an active event counts down and clears at expiry, taking
    /// its gust bonus with it.
    #[test]
    fn active_event_expires_and_clears() {
        let mut sys = WeatherSystem::new();
        sys.weather.event_id = "thunderstorm".into();
        sys.weather.event_name = "Thunderstorm".into();
        sys.weather.event_remaining_s = 5.0;
        sys.active_gust_mps = 18.0;
        let mut data = DataStore::new();
        data.insert("weather", std::sync::Mutex::new(Weather::default()));
        let mut world = hecs::World::new();
        sys.tick(&mut world, 2.0, &data);
        assert!(!sys.weather.event_id.is_empty(), "still running at t=2");
        // Gusts ride the export while active.
        let exported = data
            .get::<std::sync::Mutex<Weather>>("weather")
            .unwrap()
            .lock()
            .unwrap()
            .wind_speed;
        assert!(
            exported > sys.weather.wind_speed + 1.0,
            "export {exported} should carry the gust bonus"
        );
        sys.tick(&mut world, 10.0, &data);
        assert!(sys.weather.event_id.is_empty(), "event should have expired");
        assert_eq!(sys.active_gust_mps, 0.0);
        let after = data
            .get::<std::sync::Mutex<Weather>>("weather")
            .unwrap()
            .lock()
            .unwrap()
            .wind_speed;
        assert!((after - sys.weather.wind_speed).abs() < 1e-5, "gust gone after expiry");
    }

    /// v0.1035: with the shipped registry in the store and eligible
    /// conditions, repeated rolls eventually start an event (P(miss) per
    /// roll = 0.65; 300 rolls make a false failure astronomically rare).
    #[test]
    fn events_eventually_fire_from_the_shipped_registry() {
        let bytes = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/weather/events.ron"),
        )
        .unwrap();
        let reg =
            crate::systems::weather_events::WeatherEventRegistry::from_ron(&bytes).unwrap();
        let mut data = DataStore::new();
        data.insert("weather_event_registry", reg);
        data.insert("weather", std::sync::Mutex::new(Weather::default()));
        let mut world = hecs::World::new();
        let mut sys = WeatherSystem::new();
        // Pin ambient conditions inside the thunderstorm window each roll
        // (the normal condition machinery keeps mutating them). Temperature
        // is pinned through its DEVIATION, since the tick rebuilds the global
        // reference from base + deviation before the roll: 20 C in Spring.
        let pinned_dev = 20.0
            - body_environment::body_baseline_temp_c(&BodyEnvironment::default(), Season::Spring);
        let mut fired = false;
        for _ in 0..300 {
            sys.temp_dev = pinned_dev;
            sys.weather.wind_speed = 10.0;
            sys.weather.event_remaining_s = 0.0;
            sys.weather.event_id.clear();
            sys.event_roll_timer = 0.0;
            sys.tick(&mut world, 0.1, &data);
            if !sys.weather.event_id.is_empty() {
                fired = true;
                break;
            }
        }
        assert!(fired, "no event fired in 300 forced rolls");
    }

    #[test]
    fn exports_weather_to_datastore() {
        // Pre-seed the slot as world init does, tick, and confirm the system's
        // weather is visible in the DataStore (what the survival env + HUD read).
        let mut data = DataStore::new();
        data.insert("weather", std::sync::Mutex::new(Weather::default()));
        let mut world = hecs::World::new();
        let mut sys = WeatherSystem::new();
        sys.begin_transition(WeatherCondition::Snow, Season::Winter, QUICK_TRANSITION_S);
        for _ in 0..40 {
            sys.tick(&mut world, 1.0, &data);
        }
        let exported = data
            .get::<std::sync::Mutex<Weather>>("weather")
            .expect("weather slot")
            .lock()
            .unwrap()
            .clone();
        assert_eq!(exported.condition, sys.weather().condition);
        assert!((exported.temperature - sys.weather().temperature).abs() < 1e-6);
        // Home-default contract (review split): with no positional delta
        // published, the player-local reading IS the global one, so the
        // temperature split changes nothing at the home station.
        assert!((exported.temperature_at_player - exported.temperature).abs() < 1e-6);
        // And the home default is open space: no air pressure outside.
        assert_eq!(exported.pressure_kpa_at_player, 0.0);
    }

    /// Earth, locked, standing at `lat` degrees and `alt` metres, deep inland.
    fn earth_env(lat: f64, alt: f32, land: f32) -> BodyEnvironment {
        let la = lat.to_radians();
        BodyEnvironment {
            locked: true,
            latitude_deg: lat as f32,
            altitude_m: alt,
            up_dir: glam::DVec3::new(la.cos(), la.sin(), 0.0),
            land_fraction: land,
            ..Default::default()
        }
    }

    fn inputs<'a>(env: &'a BodyEnvironment, year_fraction: f64) -> AtPlayerInputs<'a> {
        AtPlayerInputs {
            env,
            table: ClimateTable::shipped(),
            season: Season::Summer,
            hour: 12.0,
            year_fraction,
            global_c: 30.0,
            temp_dev_c: 1.5,
            weather_wind: (0.0, 0.0),
            manual_wind: false,
        }
    }

    /// Layer 1 at the player: at the same moment and under the same weather,
    /// the equator, a mountain on the equator and the pole feel different air,
    /// and the weather's deviation rides on top of every one of them. Seen red
    /// by sampling Layer 1 at altitude 0 whatever the player's height (the
    /// equator and 4 km up on it both read 30.3 C and 101.325 kPa), and
    /// separately by breaking the wind basis (the trades vanished).
    #[test]
    fn the_equator_a_mountain_and_the_pole_feel_different_air() {
        let summer = 0.35; // northern land peak lands near here in the game year
        let eq = earth_env(0.0, 0.0, 1.0);
        let peak = earth_env(0.0, 4_000.0, 1.0);
        let pole = earth_env(85.0, 0.0, 0.0);
        let (a, b, c) = (
            air_at_player(&inputs(&eq, summer)),
            air_at_player(&inputs(&peak, summer)),
            air_at_player(&inputs(&pole, summer)),
        );
        assert!(a.temp_c > b.temp_c + 20.0, "equator {a:?} vs 4 km on it {b:?}");
        assert!(a.temp_c > c.temp_c + 15.0, "equator {a:?} vs the pole {c:?}");
        assert!(b.pressure_kpa < 0.7 * a.pressure_kpa, "thin air at 4 km: {b:?}");
        assert!((a.pressure_kpa - 101.325).abs() < 0.01, "sea level: {a:?}");
        // The deviation is exactly additive on Layer 1.
        let mut warmer = inputs(&eq, summer);
        warmer.temp_dev_c += 4.0;
        assert!((air_at_player(&warmer).temp_c - a.temp_c - 4.0).abs() < 1e-4);
        // Trade winds blow at the tropics even under a still weather roll.
        let tropic = earth_env(16.0, 0.0, 0.0);
        let t = air_at_player(&inputs(&tropic, summer));
        assert!(t.wind_east < -0.5, "trades at 16 N: {t:?}");
        // Away from any world (the home station): the global reference exactly,
        // no pressure, only the weather's wind.
        let home = BodyEnvironment::default();
        let mut i = inputs(&home, summer);
        i.weather_wind = (3.0, 0.0);
        let h = air_at_player(&i);
        assert_eq!(h.temp_c, 30.0);
        assert_eq!(h.pressure_kpa, 0.0);
        assert_eq!((h.wind_east, h.wind_north), (3.0, 0.0));
    }

    /// The F11 panel pins the local wind: "calm" has to mean calm even in the
    /// trades. Without the panel the prevailing wind adds to the weather's.
    /// Seen red by dropping the `manual_wind` branch: the panel's calm read the
    /// trade wind, (-1.40, +0.21) m/s.
    #[test]
    fn the_weather_panel_pins_the_local_wind() {
        let tropic = earth_env(16.0, 0.0, 0.0);
        let mut i = inputs(&tropic, 0.35);
        i.weather_wind = (0.0, 0.0);
        let free = air_at_player(&i);
        i.manual_wind = true;
        let pinned = air_at_player(&i);
        assert!(free.wind_east.hypot(free.wind_north) > 1.0);
        assert_eq!((pinned.wind_east, pinned.wind_north), (0.0, 0.0));
    }

    /// Mars has a row: its air is Layer 1's thin CO2 column, and its
    /// temperature carries its own body-wide day/night swing.
    #[test]
    fn mars_reads_its_own_column() {
        let mut mars = BodyEnvironment::dry_atmosphere("mars", 210.0);
        mars.altitude_m = 0.0;
        let i = AtPlayerInputs { hour: 14.0, ..inputs(&mars, 0.2) };
        let m = air_at_player(&i);
        assert!((m.pressure_kpa - 0.636).abs() < 1e-4, "Mars datum pressure {m:?}");
        assert!(m.temp_c < -20.0, "Mars is cold even at its afternoon peak: {m:?}");
        // The Moon has no row and no air.
        let moon = BodyEnvironment::airless("moon", 220.0);
        let n = air_at_player(&inputs(&moon, 0.2));
        assert_eq!(n.pressure_kpa, 0.0);
    }

    #[test]
    fn test_season_conditions() {
        let mut system = WeatherSystem::new();
        // Just verify pick_condition returns valid variants for every season
        for season in [Season::Spring, Season::Summer, Season::Autumn, Season::Winter] {
            for _ in 0..20 {
                let _ = system.pick_condition(season);
            }
        }
    }

    /// Increment 4: on an airless body (the Moon) the weather sim never
    /// produces ANY weather. Forced rolls across every season come back
    /// Clear, and the wind/humidity targets settle to zero. This is the
    /// "it can rain on the Moon" audit finding, closed.
    ///
    /// Review fix (a check that could not fail): the first version of
    /// this test had NO WeatherEventRegistry in its DataStore, so the
    /// event branch could never fire regardless of the body gate and the
    /// event_id assertion was vacuous. Now the store carries a real
    /// registry with one ALWAYS-eligible event (every season, any
    /// temperature, any wind), so ONLY the body gate stands between it
    /// and firing, and the Earth control arm at the bottom proves the
    /// same registry genuinely fires where the body allows it. Red-green
    /// proven: removing the body gate on the event roll makes the moon
    /// arm fail on "event fired on airless body".
    #[test]
    fn no_weather_at_all_on_airless_bodies() {
        use crate::systems::body_environment::BodyEnvironment;
        use crate::systems::weather_events::{
            CloudOverride, Hazard, Range, WeatherEvent, WeatherEventRegistry, WindProfile,
        };
        let mut data = DataStore::new();
        data.insert("weather", std::sync::Mutex::new(Weather::default()));
        data.insert("body_environment", BodyEnvironment::airless("moon", 220.0));
        data.insert(
            "weather_event_registry",
            WeatherEventRegistry {
                events: vec![WeatherEvent {
                    id: "test_always_eligible".to_string(),
                    name: "Test Always Eligible".to_string(),
                    seasons: vec![
                        "Spring".to_string(),
                        "Summer".to_string(),
                        "Autumn".to_string(),
                        "Winter".to_string(),
                    ],
                    temp_c: Range { min: -1000.0, max: 1000.0 },
                    wind_mps: Range { min: 0.0, max: 1000.0 },
                    rarity_weight: 1.0,
                    duration_s: Range { min: 60.0, max: 120.0 },
                    wind_profile: WindProfile::None,
                    emitters: vec!["rain".to_string()],
                    cloud: CloudOverride { coverage_boost: 0.0, tint: (1.0, 1.0, 1.0) },
                    hazard: Hazard { damage_per_s: 0.0, radius_m: 0.0 },
                }],
            },
        );
        let mut world = hecs::World::new();
        let mut sys = WeatherSystem::new();
        // Force a weather-change roll every tick, across long simulated time.
        for i in 0..200 {
            sys.next_change_timer = 0.0;
            // Also force event rolls to prove they are body-gated too.
            sys.event_roll_timer = 0.0;
            sys.tick(&mut world, 1.0, &data);
            assert_eq!(
                sys.weather().condition,
                WeatherCondition::Clear,
                "roll {i} produced weather on an airless body"
            );
            assert!(sys.weather().event_id.is_empty(), "event fired on airless body");
        }
        // After the transition settles: no wind, no humidity, full visibility.
        for _ in 0..40 {
            sys.tick(&mut world, 1.0, &data);
        }
        assert!(sys.weather().wind_speed.abs() < 0.01, "no air = no wind");
        assert!(sys.weather().humidity.abs() < 0.01, "no air = no humidity");
        assert!((sys.weather().visibility - 1.0).abs() < 0.01);

        // CONTROL ARM: the exact same registry fires on a body that CAN
        // host events (the Earth home default), proving the moon arm's
        // silence above came from the body gate and not from a registry
        // that never loaded (the vacuous-check failure mode this review
        // fix closes). 200 forced rolls at the 35% fire chance make a
        // false failure astronomically rare (0.65^200).
        data.insert("body_environment", BodyEnvironment::default());
        let mut earth_sys = WeatherSystem::new();
        let mut fired = false;
        for _ in 0..200 {
            earth_sys.event_roll_timer = 0.0;
            earth_sys.tick(&mut world, 1.0, &data);
            if !earth_sys.weather().event_id.is_empty() {
                fired = true;
                break;
            }
        }
        assert!(fired, "control arm: the always-eligible event never fired on Earth");
    }

    /// Increment 4: a body with air but no surface water (Mars) never
    /// rains, snows, or fogs; storms sanitize to dust storms.
    #[test]
    fn dry_atmosphere_never_precipitates() {
        use crate::systems::body_environment::BodyEnvironment;
        let mut data = DataStore::new();
        data.insert("weather", std::sync::Mutex::new(Weather::default()));
        data.insert("body_environment", BodyEnvironment::dry_atmosphere("mars", 210.0));
        let mut world = hecs::World::new();
        let mut sys = WeatherSystem::new();
        for i in 0..300 {
            sys.next_change_timer = 0.0;
            sys.tick(&mut world, 1.0, &data);
            let c = sys.weather().condition;
            assert!(
                !matches!(
                    c,
                    WeatherCondition::Rain
                        | WeatherCondition::Snow
                        | WeatherCondition::Fog
                        | WeatherCondition::Storm
                ),
                "roll {i} produced water weather {c:?} on a dry world"
            );
        }
    }

    /// Increment 4: hopping from the Earth home frame to the Moon retunes
    /// the temperature to the new body immediately (one transition, not a
    /// 5-15 minute roll wait). At lunar night the exported temperature is
    /// brutally cold; the internal Earth value never was.
    #[test]
    fn body_switch_retunes_temperature() {
        use crate::systems::body_environment::BodyEnvironment;
        use crate::systems::time::GameTime;
        let mut data = DataStore::new();
        data.insert("weather", std::sync::Mutex::new(Weather::default()));
        // Pin the clock at 02:00 (deep night) so the airless diurnal swing
        // is at its coldest and deterministic.
        let night = GameTime {
            hour: 2.0,
            ..Default::default()
        };
        data.insert("game_time", std::sync::Mutex::new(night));
        let mut world = hecs::World::new();
        let mut sys = WeatherSystem::new();
        // Settle on Earth (default env; no body_environment inserted yet).
        for _ in 0..40 {
            sys.tick(&mut world, 1.0, &data);
        }
        let earth_temp = data
            .get::<std::sync::Mutex<Weather>>("weather")
            .unwrap()
            .lock()
            .unwrap()
            .temperature;
        // Arrive at the Moon: the body change forces a transition NOW.
        data.insert("body_environment", BodyEnvironment::airless("moon", 220.0));
        for _ in 0..40 {
            sys.tick(&mut world, 1.0, &data);
        }
        let moon_temp = data
            .get::<std::sync::Mutex<Weather>>("weather")
            .unwrap()
            .lock()
            .unwrap()
            .temperature;
        assert!(
            moon_temp < -100.0,
            "lunar night should be brutal, got {moon_temp} (was {earth_temp} on Earth)"
        );
        assert!(
            earth_temp > -50.0,
            "Earth default weather should stay temperate, got {earth_temp}"
        );
    }
}

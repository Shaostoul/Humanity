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
    /// Current weather condition.
    pub condition: WeatherCondition,
    /// Weather intensity (0.0 = calm, 1.0 = extreme).
    pub intensity: f32,
    /// Wind speed in m/s.
    pub wind_speed: f32,
    /// Normalized wind direction vector.
    pub wind_direction: Vec3,
    /// Temperature in Celsius.
    pub temperature: f32,
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

impl Default for Weather {
    fn default() -> Self {
        Self {
            condition: WeatherCondition::Clear,
            intensity: 0.0,
            wind_speed: 2.0,
            wind_direction: Vec3::new(1.0, 0.0, 0.0).normalize(),
            temperature: 20.0,
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

/// Duration of smooth transition between weather conditions (seconds).
const TRANSITION_DURATION: f32 = 30.0;

/// Minimum game-time seconds between weather changes (5 minutes).
const MIN_CHANGE_INTERVAL: f32 = 300.0;

/// Maximum game-time seconds between weather changes (15 minutes).
const MAX_CHANGE_INTERVAL: f32 = 900.0;

/// Seconds between extreme-event roll attempts (v0.1035; same clock as
/// the change intervals above).
const EVENT_ROLL_INTERVAL_S: f32 = 900.0;

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
    prev_temperature: f32,
    prev_humidity: f32,
    prev_wind_speed: f32,
    /// Target values for the new condition.
    target_intensity: f32,
    target_visibility: f32,
    target_temperature: f32,
    target_humidity: f32,
    target_wind_speed: f32,
    /// Countdown until the next weather change attempt.
    next_change_timer: f32,
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
        Self {
            prev_intensity: weather.intensity,
            prev_visibility: weather.visibility,
            prev_temperature: weather.temperature,
            prev_humidity: weather.humidity,
            prev_wind_speed: weather.wind_speed,
            target_intensity: weather.intensity,
            target_visibility: weather.visibility,
            target_temperature: weather.temperature,
            target_humidity: weather.humidity,
            target_wind_speed: weather.wind_speed,
            weather,
            next_change_timer: 60.0, // First change after 1 minute
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
        // Per-body temperature baseline (increment 4): Earth keeps its
        // calibrated seasonal table, every other body starts from its
        // catalog mean temperature. Latitude/altitude/day-night ride on
        // top at EXPORT time (see the tick's export block) so they track
        // the player instantly instead of waiting out a 30 s transition.
        let base_temp = body_environment::body_baseline_temp_c(&self.env, season);

        // Add some random variance to temperature (+/- 5 degrees)
        let temp_variance: f32 = self.rng.gen_range(-5.0..5.0);

        match condition {
            WeatherCondition::Clear => {
                self.target_intensity = 0.0;
                self.target_visibility = 1.0;
                self.target_temperature = base_temp + temp_variance + 3.0; // Clear = slightly warmer
                self.target_humidity = 0.3 + self.rng.gen_range(0.0..0.1);
                self.target_wind_speed = self.rng.gen_range(0.5..3.0);
            }
            WeatherCondition::Cloudy => {
                self.target_intensity = self.rng.gen_range(0.2..0.5);
                self.target_visibility = 0.8;
                self.target_temperature = base_temp + temp_variance;
                self.target_humidity = 0.5 + self.rng.gen_range(0.0..0.2);
                self.target_wind_speed = self.rng.gen_range(2.0..6.0);
            }
            WeatherCondition::Rain => {
                self.target_intensity = self.rng.gen_range(0.4..0.8);
                self.target_visibility = 0.6;
                self.target_temperature = base_temp + temp_variance - 3.0; // Rain cools
                self.target_humidity = 0.8 + self.rng.gen_range(0.0..0.2);
                self.target_wind_speed = self.rng.gen_range(3.0..8.0);
            }
            WeatherCondition::Storm => {
                self.target_intensity = self.rng.gen_range(0.8..1.0);
                self.target_visibility = 0.4;
                self.target_temperature = base_temp + temp_variance - 5.0;
                self.target_humidity = 0.9 + self.rng.gen_range(0.0..0.1);
                self.target_wind_speed = self.rng.gen_range(10.0..20.0);
            }
            WeatherCondition::Snow => {
                self.target_intensity = self.rng.gen_range(0.3..0.7);
                self.target_visibility = 0.5;
                self.target_temperature = (base_temp + temp_variance).min(0.0); // Must be freezing
                self.target_humidity = 0.7 + self.rng.gen_range(0.0..0.2);
                self.target_wind_speed = self.rng.gen_range(2.0..7.0);
            }
            WeatherCondition::Fog => {
                self.target_intensity = self.rng.gen_range(0.5..0.9);
                self.target_visibility = 0.2;
                self.target_temperature = base_temp + temp_variance - 1.0;
                self.target_humidity = 0.9 + self.rng.gen_range(0.0..0.1);
                self.target_wind_speed = self.rng.gen_range(0.0..2.0);
            }
            WeatherCondition::Sandstorm => {
                self.target_intensity = self.rng.gen_range(0.6..1.0);
                self.target_visibility = 0.3;
                self.target_temperature = base_temp + temp_variance + 5.0; // Hot
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
    fn begin_transition(&mut self, new_condition: WeatherCondition, season: Season) {
        // Snapshot current values for lerping
        self.prev_intensity = self.weather.intensity;
        self.prev_visibility = self.weather.visibility;
        self.prev_temperature = self.weather.temperature;
        self.prev_humidity = self.weather.humidity;
        self.prev_wind_speed = self.weather.wind_speed;

        self.weather.condition = new_condition;
        self.weather.transition_timer = TRANSITION_DURATION;
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
        let (season, hour) = data
            .get::<std::sync::Mutex<GameTime>>("game_time")
            .and_then(|m| m.lock().ok())
            .map(|gt| (gt.season, gt.hour))
            .unwrap_or((Season::Spring, 12.0));

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
            self.begin_transition(cond, season);
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
                self.begin_transition(m.condition, season);
            }
        }

        // Count down to next weather change
        self.next_change_timer -= dt;
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
                self.begin_transition(new_condition, season);
            }
            // Schedule next change
            self.next_change_timer = self.rng.gen_range(MIN_CHANGE_INTERVAL..MAX_CHANGE_INTERVAL);
        }

        // Process smooth transition
        if self.weather.transition_timer > 0.0 {
            self.weather.transition_timer = (self.weather.transition_timer - dt).max(0.0);
            let t = 1.0 - (self.weather.transition_timer / TRANSITION_DURATION);
            // Smooth-step for more natural transitions
            let t = t * t * (3.0 - 2.0 * t);

            self.weather.intensity = lerp(self.prev_intensity, self.target_intensity, t);
            self.weather.visibility = lerp(self.prev_visibility, self.target_visibility, t);
            self.weather.temperature = lerp(self.prev_temperature, self.target_temperature, t);
            self.weather.humidity = lerp(self.prev_humidity, self.target_humidity, t);
            self.weather.wind_speed = lerp(self.prev_wind_speed, self.target_wind_speed, t);
        }

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
            self.weather.event_remaining_s = (self.weather.event_remaining_s - dt).max(0.0);
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
            self.event_roll_timer -= dt;
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

        // Export the current weather to the DataStore so the survival environment
        // (the exposed ambient temperature) and the HUD read it. Interior mutability
        // via a Mutex (the TimeSystem/game_time pattern) since tick only gets &DataStore.
        // Front-event gusts ride the EXPORT only, so the internal lerp
        // targets stay clean and event wind vanishes with the event.
        // The positional temperature delta (latitude, altitude, day/night;
        // increment 4) ALSO rides the export only: it tracks the player's
        // position instantly, while the internal value keeps ramping the
        // body baseline + condition offsets through transitions. It is
        // exactly 0.0 for the default Earth home environment.
        if let Some(slot) = data.get::<std::sync::Mutex<Weather>>("weather") {
            if let Ok(mut w) = slot.lock() {
                *w = self.weather.clone();
                w.temperature = self.weather.temperature
                    + body_environment::positional_temp_offset_c(&self.env, season, hour);
                w.wind_speed += self.active_gust_mps * EVENT_GUST_EXPORT;
            }
        }
    }
}

/// Linear interpolation.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
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
        // (the normal condition machinery keeps mutating them).
        let mut fired = false;
        for _ in 0..300 {
            sys.weather.temperature = 20.0;
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
        sys.begin_transition(WeatherCondition::Snow, Season::Winter);
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
    #[test]
    fn no_weather_at_all_on_airless_bodies() {
        use crate::systems::body_environment::BodyEnvironment;
        let mut data = DataStore::new();
        data.insert("weather", std::sync::Mutex::new(Weather::default()));
        data.insert("body_environment", BodyEnvironment::airless("moon", 220.0));
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

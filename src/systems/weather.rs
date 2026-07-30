//! Weather system — dynamic weather simulation driven by season and randomness.
//!
//! Stores `Weather` in the WeatherSystem struct. Other systems can read
//! weather state to affect farming, visibility, combat, etc.

use glam::Vec3;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
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
        let base_temp = match season {
            Season::Spring => 15.0,
            Season::Summer => 30.0,
            Season::Autumn => 12.0,
            Season::Winter => -2.0,
        };

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
        // Determine current season from the GameTime that TimeSystem exports into
        // the DataStore (behind a Mutex); fall back to Spring if absent.
        let season = data
            .get::<std::sync::Mutex<GameTime>>("game_time")
            .and_then(|m| m.lock().ok())
            .map(|gt| gt.season)
            .unwrap_or(Season::Spring);

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
            let new_condition = self.pick_condition(season);
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
        } else if manual.is_none() {
            // No random extreme events while the F11 panel is driving.
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
        if let Some(slot) = data.get::<std::sync::Mutex<Weather>>("weather") {
            if let Ok(mut w) = slot.lock() {
                *w = self.weather.clone();
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

        // Tick a few times — should not panic
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
}

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
        }
    }
}

/// Duration of smooth transition between weather conditions (seconds).
const TRANSITION_DURATION: f32 = 30.0;

/// Minimum game-time seconds between weather changes (5 minutes).
const MIN_CHANGE_INTERVAL: f32 = 300.0;

/// Maximum game-time seconds between weather changes (15 minutes).
const MAX_CHANGE_INTERVAL: f32 = 900.0;

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

    fn tick(&mut self, _world: &mut hecs::World, dt: f32, _data: &DataStore) {
        // Determine current season from game time (read from DataStore if available,
        // otherwise default to Spring). WeatherSystem reads GameTime but TimeSystem
        // stores it internally, so we fall back gracefully.
        let season = _data
            .get::<GameTime>("game_time")
            .map(|gt| gt.season)
            .unwrap_or(Season::Spring);

        // Count down to next weather change
        self.next_change_timer -= dt;
        if self.next_change_timer <= 0.0 {
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

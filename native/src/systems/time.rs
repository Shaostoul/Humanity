//! Time system — day/night cycle, seasons, and sun position.
//!
//! Stores `GameTime` in DataStore under key "game_time".
//! Computes sun direction and color for the renderer.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// In-game season derived from day count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    /// Determine season from day count (30-day seasons, 120-day year).
    pub fn from_day(day: u32) -> Self {
        match (day % 120) / 30 {
            0 => Season::Spring,
            1 => Season::Summer,
            2 => Season::Autumn,
            _ => Season::Winter,
        }
    }
}

/// Complete game time state — serializable for save/load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameTime {
    /// Total elapsed game seconds since world creation.
    pub elapsed_seconds: f64,
    /// Number of full days completed.
    pub day_count: u32,
    /// Current hour of the day (0.0 .. 24.0).
    pub hour: f32,
    /// Current season.
    pub season: Season,
    /// Time multiplier (1.0 = real-time, higher = faster).
    pub time_scale: f32,
}

impl Default for GameTime {
    fn default() -> Self {
        Self {
            elapsed_seconds: 0.0,
            day_count: 0,
            hour: 8.0,
            season: Season::Spring,
            time_scale: 1.0,
        }
    }
}

/// Seconds per in-game day (real-time at time_scale=1.0).
/// 20 real minutes = 1 game day.
const SECONDS_PER_DAY: f64 = 1200.0;

/// Drives the day/night cycle and writes sun parameters to DataStore.
pub struct TimeSystem {
    game_time: GameTime,
    initialized: bool,
}

impl TimeSystem {
    pub fn new() -> Self {
        Self {
            game_time: GameTime::default(),
            initialized: false,
        }
    }

    /// Compute sun direction from hour of day.
    /// Sun rises at 6, peaks at 12, sets at 18, below horizon at night.
    fn sun_direction(hour: f32) -> Vec3 {
        // Map hour to angle: 6h = 0 (horizon), 12h = PI/2 (zenith), 18h = PI (horizon)
        let day_fraction = (hour - 6.0) / 12.0; // 0 at sunrise, 1 at sunset
        let angle = day_fraction * std::f32::consts::PI;

        if hour >= 6.0 && hour <= 18.0 {
            // Daytime: sun arcs from east (+X) to west (-X), peaking at Y=1
            let y = angle.sin();
            let x = angle.cos();
            Vec3::new(x, y.max(0.01), -0.3).normalize()
        } else {
            // Nighttime: sun below horizon, provide faint moonlight direction
            Vec3::new(0.0, -0.5, -0.5).normalize()
        }
    }

    /// Compute sun color based on hour — warm at dawn/dusk, white at noon, dark at night.
    fn sun_color(hour: f32) -> [f32; 3] {
        if hour < 5.0 || hour > 19.5 {
            // Deep night — faint blue moonlight
            [0.05, 0.05, 0.1]
        } else if hour < 6.5 {
            // Dawn — orange/pink
            let t = (hour - 5.0) / 1.5;
            [0.8 * t + 0.1, 0.3 * t + 0.05, 0.1 * t + 0.05]
        } else if hour < 8.0 {
            // Morning — warming to white
            let t = (hour - 6.5) / 1.5;
            let r = 0.9 + 0.1 * t;
            let g = 0.35 + 0.55 * t;
            let b = 0.15 + 0.75 * t;
            [r, g, b]
        } else if hour < 16.0 {
            // Full daylight — warm white
            [1.0, 0.95, 0.9]
        } else if hour < 18.0 {
            // Evening — cooling toward golden hour
            let t = (hour - 16.0) / 2.0;
            [1.0, 0.95 - 0.5 * t, 0.9 - 0.7 * t]
        } else {
            // Dusk — fading orange to night
            let t = (hour - 18.0) / 1.5;
            let r = (0.9 * (1.0 - t)).max(0.05);
            let g = (0.4 * (1.0 - t)).max(0.05);
            let b = (0.2 * (1.0 - t)).max(0.1);
            [r, g, b]
        }
    }
}

impl System for TimeSystem {
    fn name(&self) -> &str {
        "TimeSystem"
    }

    fn tick(&mut self, _world: &mut hecs::World, dt: f32, _data: &DataStore) {
        let scaled_dt = dt as f64 * self.game_time.time_scale as f64;
        self.game_time.elapsed_seconds += scaled_dt;

        // Calculate current hour from total elapsed seconds
        let day_seconds = self.game_time.elapsed_seconds % SECONDS_PER_DAY;
        self.game_time.hour = (day_seconds / SECONDS_PER_DAY * 24.0) as f32;

        // Calculate day count
        self.game_time.day_count = (self.game_time.elapsed_seconds / SECONDS_PER_DAY) as u32;

        // Determine season
        self.game_time.season = Season::from_day(self.game_time.day_count);

        self.initialized = true;
    }
}

impl TimeSystem {
    /// Get current game time (for systems that need to read it directly).
    pub fn game_time(&self) -> &GameTime {
        &self.game_time
    }

    /// Get current sun direction.
    pub fn current_sun_direction(&self) -> Vec3 {
        Self::sun_direction(self.game_time.hour)
    }

    /// Get current sun color.
    pub fn current_sun_color(&self) -> [f32; 3] {
        Self::sun_color(self.game_time.hour)
    }

    /// Set time scale (speed multiplier).
    pub fn set_time_scale(&mut self, scale: f32) {
        self.game_time.time_scale = scale.max(0.0);
    }

    /// Jump to a specific hour (0-24).
    pub fn set_hour(&mut self, hour: f32) {
        let clamped = hour.rem_euclid(24.0);
        let current_day_start = (self.game_time.day_count as f64) * SECONDS_PER_DAY;
        self.game_time.elapsed_seconds = current_day_start + (clamped as f64 / 24.0) * SECONDS_PER_DAY;
        self.game_time.hour = clamped;
    }

    /// Check if it's currently daytime (between 6:00 and 18:00).
    pub fn is_daytime(&self) -> bool {
        self.game_time.hour >= 6.0 && self.game_time.hour <= 18.0
    }
}

//! Solar system — drives solar `PowerGenerator` output from the time of day.
//!
//! Runs BEFORE `ElectricalSystem` each frame: for every entity that is both a
//! `PowerGenerator` and a `SolarPanel`, set `output_watts = peak_watts * sun_factor(hour)`,
//! so generation climbs from zero at sunrise to the nameplate peak at noon and back to
//! zero at sunset. `ElectricalSystem` then sums the scaled output like any other generator.
//!
//! This is the first piece of the LIVE home simulation: the home's solar generation is no
//! longer a hardcoded string, it moves with the sun. Reads the hour from the `game_time`
//! Mutex in the DataStore (the same shared-state pattern `TimeSystem` writes to).

use crate::ecs::components::{PowerGenerator, SolarPanel};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::time::GameTime;

/// Fraction of nameplate solar output at a given hour (0.0 at night, 1.0 at noon).
/// Matches the sun arc in `TimeSystem`: up from 6h to 18h, peaking at noon.
pub fn sun_factor(hour: f32) -> f32 {
    if (6.0..=18.0).contains(&hour) {
        (((hour - 6.0) / 12.0) * std::f32::consts::PI).sin().max(0.0)
    } else {
        0.0
    }
}

pub struct SolarSystem;

impl SolarSystem {
    pub fn new() -> Self {
        Self
    }
}

impl System for SolarSystem {
    fn name(&self) -> &str {
        "SolarSystem"
    }

    fn tick(&mut self, world: &mut hecs::World, _dt: f32, data: &DataStore) {
        // Current hour from the shared GameTime (same Mutex pattern as TimeSystem).
        let hour = data
            .get::<std::sync::Mutex<GameTime>>("game_time")
            .and_then(|m| m.lock().ok().map(|t| t.hour))
            .unwrap_or(12.0);
        let factor = sun_factor(hour);
        for (_e, (gen, panel)) in world.query::<(&mut PowerGenerator, &SolarPanel)>().iter() {
            gen.output_watts = panel.peak_watts * factor;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sun_factor_curve() {
        assert!((sun_factor(12.0) - 1.0).abs() < 1e-5, "peak at noon");
        assert_eq!(sun_factor(6.0), 0.0, "zero at sunrise");
        assert_eq!(sun_factor(18.0), 0.0, "zero at sunset");
        assert_eq!(sun_factor(0.0), 0.0, "zero at midnight");
        assert_eq!(sun_factor(23.0), 0.0, "zero late night");
        // Mid-morning is partial.
        let nine = sun_factor(9.0);
        assert!(nine > 0.6 && nine < 0.8, "9am ~0.707, got {nine}");
    }
}

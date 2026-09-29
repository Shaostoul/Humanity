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

/// The longitude (degrees east) a planet build site stands at: its origin in
/// the body's unrotated frame, in the longitude convention the planet spin
/// uses (`atan2(-z, x)`, `dev_travel::planet_spin_from_time`).
pub fn site_longitude_deg(site: &crate::systems::construction::PlanetSite) -> f64 {
    (-site.origin.z).atan2(site.origin.x).to_degrees()
}

/// Share of the clear sky's sunlight that reaches the ground under `cloud`
/// (0 clear to 1 overcast, `Weather::cloud_share`): Kasten and Czeplak
/// (1980, Solar Energy 24, 177-189), `G / G_clear = 1 - 0.75 (N/8)^3.4` with
/// N the cloud cover in eighths of the sky. A full overcast lets through a
/// quarter; a half-covered sky still lets through about 93 percent, because
/// the broken cloud mostly misses the sun.
pub fn cloud_light_share(cloud: f32) -> f32 {
    1.0 - 0.75 * cloud.clamp(0.0, 1.0).powf(3.4)
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
        // The sun's place from the shared GameTime (same Mutex pattern as
        // TimeSystem), on a 24-hour dial so any day length rises at a quarter
        // of the day and sets at three quarters, at the longitude of where the
        // panel stands: the HOME's for a panel aboard, so the panels see the
        // sun the deck sees (BUG-090), and a planet build site's own for a
        // panel built there (2026-09-28).
        let home_lon = crate::systems::time::home_longitude_deg(data);
        let clock = data.get::<std::sync::Mutex<GameTime>>("game_time").and_then(|m| m.lock().ok().map(|t| t.clone()));
        // The weather's clouds (2026-09-28) cover the body the weather is
        // simulating: the frame-locked one, when it has air. A panel on that
        // body's ground sits under them. A panel aboard is above the weather,
        // and one on another body has no weather simulated for it, so both
        // keep a clear sky.
        let clouded_body = data
            .get::<crate::systems::body_environment::BodyEnvironment>("body_environment")
            .filter(|e| e.locked && e.has_atmosphere)
            .map(|e| e.body_id.clone());
        let cloud = data
            .get::<std::sync::Mutex<crate::systems::weather::Weather>>("weather")
            .and_then(|m| m.lock().ok().map(|w| w.cloud_share()))
            .unwrap_or(0.0);
        for (_e, (gen, panel, site)) in world
            .query::<(&mut PowerGenerator, &SolarPanel, Option<&crate::systems::construction::PlanetSite>)>()
            .iter()
        {
            let lon = site.map_or(home_lon, site_longitude_deg);
            let hour = clock.as_ref().map_or(12.0, |t| t.solar_hour_at(lon));
            let light = match site {
                Some(s) if clouded_body.as_deref() == Some(s.body.as_str()) => cloud_light_share(cloud),
                _ => 1.0,
            };
            gen.output_watts = panel.peak_watts * sun_factor(hour) * light;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CLOUDS DIM A PANEL ON THE GROUND (2026-09-28). At noon under an
    /// overcast sky (rain) on Earth, a panel built on Earth makes a quarter
    /// of its peak (Kasten and Czeplak), one built on the Moon, where no
    /// weather is simulated, makes its peak, and one aboard the home is above
    /// the weather. Red check, run: leaving the cloud share out fails the
    /// first assertion.
    #[test]
    fn clouds_dim_a_panel_on_the_ground() {
        use crate::ecs::components::{PowerGenerator, SolarPanel};
        use crate::systems::body_environment::BodyEnvironment;
        use crate::systems::construction::PlanetSite;
        use crate::systems::weather::{Weather, WeatherCondition};
        use glam::DVec3;
        let mut data = DataStore::new();
        let mut gt = GameTime::default();
        gt.set_elapsed(12.0 * 3600.0);
        data.insert("game_time", std::sync::Mutex::new(gt));
        let mut env = BodyEnvironment::default();
        env.locked = true;
        data.insert("body_environment", env);
        data.insert("weather", std::sync::Mutex::new(Weather { condition: WeatherCondition::Rain, ..Weather::default() }));
        let mut world = hecs::World::new();
        let panel = || (PowerGenerator { output_watts: 0.0, fuel_per_second: 0.0, active: true }, SolarPanel { peak_watts: 400.0 });
        let on = |body: &str| PlanetSite { body: body.into(), origin: DVec3::new(6.371e6, 0.0, 0.0) };
        let earth = world.spawn((panel().0, panel().1, on("earth")));
        let moon = world.spawn((panel().0, panel().1, on("moon")));
        let aboard = world.spawn(panel());
        SolarSystem::new().tick(&mut world, 0.1, &data);
        let w = |e| world.get::<&PowerGenerator>(e).unwrap().output_watts;
        assert!((w(earth) - 100.0).abs() < 1e-3, "overcast noon on Earth: {}", w(earth));
        assert!((w(moon) - 400.0).abs() < 1e-3, "no weather on the Moon: {}", w(moon));
        assert!((w(aboard) - 400.0).abs() < 1e-3, "above the weather: {}", w(aboard));
        assert!((cloud_light_share(0.5) - 0.929).abs() < 0.001, "half cover");
    }

    /// A PANEL SEES ITS OWN SITE'S SUN (2026-09-28). At noon on longitude 0
    /// (game hour 12), a panel built at a site on longitude 0 makes its peak,
    /// one at a site 90 degrees east is at dusk (18:00 there), and a panel
    /// aboard the home (122.3 W, 03:51 there) makes nothing. Red check, run:
    /// reading the home's longitude for every panel fails the first
    /// assertion.
    #[test]
    fn a_panel_sees_the_sun_where_it_stands() {
        use crate::ecs::components::{PowerGenerator, SolarPanel};
        use crate::systems::construction::PlanetSite;
        use glam::DVec3;
        let mut data = DataStore::new();
        let mut gt = GameTime::default();
        gt.set_elapsed(12.0 * 3600.0);
        data.insert("game_time", std::sync::Mutex::new(gt));
        data.insert(crate::systems::time::HOME_LONGITUDE_KEY, -122.3_f64);
        let mut world = hecs::World::new();
        let panel = || (PowerGenerator { output_watts: 0.0, fuel_per_second: 0.0, active: true }, SolarPanel { peak_watts: 400.0 });
        let r = 6.371e6;
        let at_lon0 = world.spawn((panel().0, panel().1, PlanetSite { body: "earth".into(), origin: DVec3::new(r, 0.0, 0.0) }));
        let at_lon90e = world.spawn((panel().0, panel().1, PlanetSite { body: "earth".into(), origin: DVec3::new(0.0, 0.0, -r) }));
        let aboard = world.spawn(panel());
        SolarSystem::new().tick(&mut world, 0.1, &data);
        let w = |e| world.get::<&PowerGenerator>(e).unwrap().output_watts;
        assert!((w(at_lon0) - 400.0).abs() < 1e-3, "noon at the site on longitude 0: {}", w(at_lon0));
        assert!(w(at_lon90e).abs() < 1e-3, "dusk 90 degrees east: {}", w(at_lon90e));
        assert_eq!(w(aboard), 0.0, "night over the home at 122.3 W");
        assert!((site_longitude_deg(&PlanetSite { body: "earth".into(), origin: DVec3::new(0.0, 0.0, -r) }) - 90.0).abs() < 1e-9);
    }

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

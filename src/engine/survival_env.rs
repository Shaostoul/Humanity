//! The survival environment context, published once a frame (moved out of
//! lib.rs 2026-09-27, when it grew the body heat inputs).
//!
//! Is the player inside the sealed home (oxygenated, the home's own air) or
//! outside (the weather, or vacuum)? Outside, are they under a shelter they
//! built (`construction::uses::shelter_at`, 2026-09-27)? And what does the body heat model
//! (`systems::body_heat`) need to know about where they stand: the air's
//! temperature, humidity and pressure, the wind, anything falling on them,
//! and what they are doing. FoodSystem reads the result from the DataStore
//! ("environment_context") to drive oxygen and the core temperature, and the
//! Settings > Gameplay > Body heat mode rides beside it (`body_heat::MODE_KEY`).

use crate::ecs::components::EnvironmentContext;
use crate::engine::state::EngineState;
use crate::systems::body_heat;
use crate::systems::construction::uses;
use crate::systems::weather::Weather;

/// Sea-level air pressure, kPa (the home air's pressure is kept in atm).
const SEA_LEVEL_KPA: f32 = 101.325;

/// The outside air where the player stands, as the weather exports it
/// (environment Layer 1 plus the weather's deviation: `systems::weather`,
/// `air_at_player`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ExposedAir {
    pub temp_c: f32,
    pub relative_humidity: f32,
    pub wind_m_s: f32,
    pub precipitation: f32,
    pub pressure_kpa: f32,
}

impl ExposedAir {
    /// No weather published yet: assume the worst (-40 C, no air).
    const UNKNOWN: ExposedAir =
        ExposedAir { temp_c: -40.0, relative_humidity: 0.0, wind_m_s: 0.0, precipitation: 0.0, pressure_kpa: 0.0 };

    /// The PLAYER-LOCAL fields of the weather, never the body-global
    /// `temperature` (review split): exposure is about where THIS body stands
    /// (latitude, altitude, lunar night), while farming climate and hydrology
    /// evaporation keep reading the global reference. See the Weather struct's
    /// field docs for the full contract.
    pub(crate) fn from_weather(w: &Weather) -> Self {
        ExposedAir {
            temp_c: w.temperature_at_player,
            relative_humidity: w.humidity,
            wind_m_s: w.wind_speed_at_player(),
            precipitation: body_heat::precipitation(w.condition, w.intensity),
            pressure_kpa: w.pressure_kpa_at_player,
        }
    }
}

/// The context OUTSIDE the hull: unsealed, in the live weather, under what
/// the player has built over themselves (`uses::shelter_at`). Breathable
/// ONLY when standing on a body whose open air supports it (Earth below the
/// death zone); space, the Moon, and Mars keep the vacuum oxygen drain
/// (artificial-planet increment 4). A roof keeps the rain and snow off even
/// with walls missing; a roof on enough walls is `sheltered`, which tells the
/// body heat model the wind does not reach the body either
/// (`body_heat::Exposure::from_context`). The air is the weather's either
/// way. Pure, so the body heat chain can be tested.
pub(crate) fn outside_context(
    air: ExposedAir,
    breathable: bool,
    shelter: uses::ShelterCheck,
    activity_met: f32,
    g_load: f32,
) -> EnvironmentContext {
    EnvironmentContext {
        sealed: false,
        oxygenated: breathable,
        ambient_temp_c: air.temp_c,
        relative_humidity: air.relative_humidity,
        wind_m_s: air.wind_m_s,
        precipitation: if shelter.roofed { 0.0 } else { air.precipitation },
        sheltered: shelter.sheltered(),
        radiant_temp_c: None,
        pressure_kpa: air.pressure_kpa,
        activity_met,
        // The drive does not care which side of the hull you are on: a burn
        // is felt everywhere aboard.
        g_load,
    }
}

/// Once per frame: publish the survival context and the body heat mode.
pub(crate) fn publish(state: &mut EngineState) {
    let mode = if state.gui_state.settings.body_heat_realistic {
        body_heat::Mode::Realistic
    } else {
        body_heat::Mode::Forgiving
    };
    state.data_store.insert(body_heat::MODE_KEY, mode);

    // The air where the player stands: temperature, humidity, wind, what is
    // falling and the pressure, all from the weather's at-player export
    // (environment Layer 1 plus the weather's deviation).
    let exposed = state
        .data_store
        .get::<std::sync::Mutex<Weather>>("weather")
        .and_then(|m| m.lock().ok())
        .map(|w| ExposedAir::from_weather(&w))
        .unwrap_or(ExposedAir::UNKNOWN);
    // Outside the hull: breathable only when standing on a frame-locked body
    // whose air is breathable at this altitude (increment 4). Open space and
    // airless or unbreathable worlds keep the vacuum drain (the existing suit
    // rules).
    let outside_breathable = state
        .data_store
        .get::<crate::systems::body_environment::BodyEnvironment>("body_environment")
        .map(|e| e.breathable_outside())
        .unwrap_or(false);
    // What the body is doing (ASHRAE met values, body_heat::MET_*).
    let activity = if state.driving_vehicle.is_some() {
        body_heat::MET_DRIVING
    } else {
        let (moving, sprinting) = state.controller.gait();
        body_heat::activity_met(moving, sprinting)
    };
    let pos = state.camera.position;
    // What the drive + any spin are doing to a body right now, after the
    // dampeners take their cut. Computed ONCE here so every branch below
    // reports the same number: a burn is felt on both sides of the hull.
    let felt = crate::systems::flight::felt_gravity(&state.flight, &state.flight_data.dampener);
    let felt_g_now = felt.felt_g;
    // Published for consumers outside this block (farming reads its own
    // DataStore slots, not EnvironmentContext).
    state.data_store.insert("felt_gravity", felt);
    // Dev fly/travel (v0.791.x) is a cheat: while fly mode is on, the
    // vacuum-outside-the-hull rule is suspended so sightseeing at Neptune
    // doesn't suffocate/freeze the operator or close-range verifies. Turning
    // fly mode off restores normal survival rules wherever you are.
    // Set in the outside branch below; nothing shelters inside the home or in fly mode.
    let mut shelter = uses::ShelterCheck::default();
    let env = if state.controller.fly_mode {
        // Fly mode already suspends vacuum and cold; suspend the burn too, so
        // sightseeing during a 5 g evasion does not quietly kill the operator.
        EnvironmentContext::default()
    } else {
        match state.homestead_bounds {
            Some((mn, mx))
                if pos.x >= mn.x && pos.x <= mx.x && pos.y >= mn.y && pos.y <= mx.y && pos.z >= mn.z && pos.z <= mx.z =>
            {
                // Inside the homestead: sealed, still air at the home's own
                // temperature and humidity, and OXYGENATED only while the
                // life-support air is breathable (v0.618). Since 2026-09-26 that
                // air is ship life support's real balance (systems::life_support):
                // the household uses about 2 kg of oxygen a day out of tonnes,
                // so losing the air machines costs months, not minutes.
                let breathable = state
                    .data_store
                    .get::<std::sync::Mutex<crate::systems::atmosphere::AirStatus>>("air_status")
                    .and_then(|m| m.lock().ok())
                    .map(|a| a.breathable)
                    .unwrap_or(true);
                let home = home_air(&state.game_world.world);
                let base = EnvironmentContext::default();
                EnvironmentContext {
                    oxygenated: breathable,
                    ambient_temp_c: home.map_or(base.ambient_temp_c, |h| h.0),
                    relative_humidity: home.map_or(base.relative_humidity, |h| h.1),
                    pressure_kpa: home.map_or(base.pressure_kpa, |h| h.2),
                    activity_met: activity,
                    // A sealed hull stops vacuum, not acceleration.
                    g_load: felt_g_now,
                    ..base
                }
            }
            Some(_) => {
                // Outside the hull, under whatever the player has built over
                // themselves (2026-09-27: a roof on three walls keeps the wind
                // and rain off). Only while the player is in the home frame the
                // pieces live in: off the ship the camera does not move with
                // the player, so testing the raw position against home-frame
                // pieces said "Sheltered" wherever one walked (BUG-102; see
                // engine/build_place.rs, which refuses to place off the ship
                // for the same reason).
                if state.aboard_station {
                    shelter = uses::shelter_at(&state.game_world.world, pos - glam::Vec3::Y * state.controller.eye_height());
                }
                outside_context(exposed, outside_breathable, shelter, activity, felt_g_now)
            }
            // Homestead not generated yet: assume safe.
            None => EnvironmentContext::default(),
        }
    };
    state.data_store.insert("environment_context", env);
    // The HUD's Shelter line and the Inventory page's readout.
    state.gui_state.vitals.sheltered = shelter.sheltered();
    state.gui_state.vitals.shelter_note = shelter.note();
}

/// THE home's air: temperature (C), relative humidity (0 to 1) and pressure
/// (kPa), from its enclosed space (`atmosphere::HomeAir`). None before the
/// home is spawned.
fn home_air(world: &hecs::World) -> Option<(f32, f32, f32)> {
    use crate::systems::atmosphere::{EnclosedSpace, HomeAir};
    world.query::<(&HomeAir, &EnclosedSpace)>().iter().next().map(|(_, (_, s))| {
        let a = &s.atmosphere;
        (a.temperature_k - 273.15, a.humidity.clamp(0.0, 1.0), a.pressure_atm * SEA_LEVEL_KPA)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hot_reload::data_store::DataStore;
    use crate::systems::body_environment::BodyEnvironment;
    use crate::systems::weather::WeatherSystem;
    use crate::ecs::systems::System;

    /// The body heat INPUT at the player changes with altitude: at the same
    /// place and moment, under the same weather, a mountain top is colder and
    /// thinner than the shore below it, and an hour standing there cools a body
    /// more. The whole chain: WeatherSystem's export (environment Layer 1 plus
    /// the weather's deviation) -> ExposedAir -> the outside context -> the
    /// body heat model. Seen red by exporting Layer 1 at altitude 0 whatever
    /// the player's height: the summit and the shore then read the same air,
    /// 7.45 C and 101.325 kPa.
    #[test]
    fn a_mountain_top_is_colder_than_the_shore_for_the_body() {
        let mut data = DataStore::new();
        data.insert("weather", std::sync::Mutex::new(Weather::default()));
        let at = |alt: f32| {
            let la = 46f64.to_radians();
            BodyEnvironment {
                locked: true,
                latitude_deg: 46.0,
                altitude_m: alt,
                up_dir: glam::DVec3::new(la.cos(), la.sin(), 0.0),
                land_fraction: 1.0,
                ..Default::default()
            }
        };
        let mut world = hecs::World::new();
        let mut sys = WeatherSystem::new();
        let mut read = |alt: f32, sys: &mut WeatherSystem| {
            data.insert("body_environment", at(alt));
            // dt 0: the weather itself does not move between the two reads,
            // so the difference is Layer 1's alone.
            sys.tick(&mut world, 0.0, &data);
            let w = data.get::<std::sync::Mutex<Weather>>("weather").unwrap().lock().unwrap().clone();
            ExposedAir::from_weather(&w)
        };
        let shore = read(0.0, &mut sys);
        let summit = read(3_000.0, &mut sys);
        // 6.5 K per geopotential km: 2,998.6 m of it.
        let drop = shore.temp_c - summit.temp_c;
        assert!((drop - 19.49).abs() < 0.05, "3 km of lapse: {drop} C (shore {shore:?}, summit {summit:?})");
        assert!(summit.pressure_kpa < 0.72 * shore.pressure_kpa, "summit air is thin: {summit:?}");
        assert!(shore.pressure_kpa > 100.0, "the shore is at sea level: {shore:?}");

        // And the body feels it: an hour standing in each, same clothes.
        let body_after_an_hour = |air: ExposedAir| {
            let ctx = outside_context(air, true, uses::ShelterCheck::default(), body_heat::MET_STANDING, 1.0);
            let ex = body_heat::Exposure::from_context(&ctx);
            let mut b = body_heat::BodyHeat::new(body_heat::CORE_NEUTRAL_C);
            b.step(&ex, body_heat::BASE_OUTFIT_CLO, body_heat::MET_STANDING, 3_600.0);
            b
        };
        let (low, high) = (body_after_an_hour(shore), body_after_an_hour(summit));
        assert!(
            high.skin_c < low.skin_c - 1.0,
            "an hour on the summit cools the skin more: shore {} C, summit {} C",
            low.skin_c,
            high.skin_c
        );
    }

    /// THE SHELTER REACHES THE BODY (2026-09-27). A wet, windy 5 C day (a
    /// 3.3 m/s wind and a downpour, the body-heat doc's "5 C, soaked by rain"
    /// case), standing in the everyday outfit for six hours: under a roof
    /// built on three walls the wind and the rain are gone from the body
    /// heat inputs and the core holds 36.66 C, where in the open, soaked, it
    /// has fallen to 36.28 C (and reaches 35 C at 7.3 h, per
    /// docs/design/body-heat.md). The whole chain runs: built pieces placed as the build menu
    /// places them, `uses::shelter_at`, `outside_context`,
    /// `Exposure::from_context`, the body model. A roof with walls missing
    /// keeps the rain off but not the wind. Red check, run: making
    /// `outside_context` set `sheltered: false` (what `publish` did before)
    /// leaves the wind and rain on the sheltered body and the first
    /// assertion fails.
    #[test]
    fn a_built_shelter_keeps_a_wet_windy_5c_day_off_the_body() {
        use crate::systems::body_heat::{BodyHeat, Exposure, BASE_OUTFIT_CLO, MET_STANDING};
        use crate::systems::construction::{placement, BlueprintRegistry, Structure};
        use glam::Vec3;
        let reg = BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap();
        let mut world = hecs::World::new();
        for (id, x, z, turns) in [("wood_wall", 0.0, -2.0, 0), ("wood_wall", -2.0, 0.0, 1), ("wood_wall", 2.0, 0.0, 1), ("roof", 0.0, 0.0, 0)] {
            let bp = reg.get(id).unwrap();
            let tf = placement::placement_pose(bp, Vec3::new(x, 0.0, z), turns, &world, &reg);
            world.spawn((tf, Structure { blueprint_id: id.into(), health: bp.health, max_health: bp.health, provides: bp.provides.clone(), uid: 0 }));
        }
        let under = uses::shelter_at(&world, Vec3::new(0.0, 0.0, 0.5));
        let open = uses::shelter_at(&world, Vec3::new(8.0, 0.0, 0.0));
        assert!(under.sheltered() && !open.sheltered(), "{under:?} {open:?}");

        let weather = ExposedAir {
            temp_c: 5.0,
            relative_humidity: 0.9,
            wind_m_s: 3.3,
            precipitation: 1.0,
            pressure_kpa: SEA_LEVEL_KPA,
        };
        let six_hours = |check: uses::ShelterCheck| {
            let ex = Exposure::from_context(&outside_context(weather, true, check, MET_STANDING, 1.0));
            let mut body = BodyHeat::new(37.0);
            for _ in 0..360 {
                body.step(&ex, BASE_OUTFIT_CLO, MET_STANDING, 60.0);
            }
            (ex, body)
        };
        let (ex_under, body_under) = six_hours(under);
        let (ex_open, body_open) = six_hours(open);
        assert_eq!((ex_under.wind_10m_m_s, ex_under.precipitation), (0.0, 0.0), "no wind or rain under the shelter");
        assert_eq!((ex_open.wind_10m_m_s, ex_open.precipitation), (3.3, 1.0), "both in the open");
        assert_eq!(ex_under.air_c, 5.0, "a shelter with no fire is still 5 C inside");
        assert_eq!(body_under.clothing_wetness, 0.0, "dry under the roof");
        assert!(body_open.clothing_wetness > 0.5, "soaked in the open: {}", body_open.clothing_wetness);
        assert!(
            body_under.core_c > body_open.core_c + 0.25,
            "sheltered core {:.2} C, open {:.2} C after 6 h",
            body_under.core_c,
            body_open.core_c
        );

        // A roof with walls missing still keeps the rain off; the wind gets in.
        let roof_only = uses::ShelterCheck { roofed: true, walled_sides: 2 };
        let ex = Exposure::from_context(&outside_context(weather, true, roof_only, MET_STANDING, 1.0));
        assert_eq!((ex.wind_10m_m_s, ex.precipitation), (3.3, 0.0), "rain off, wind in");
    }
}

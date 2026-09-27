//! The survival environment context, published once a frame (moved out of
//! lib.rs 2026-09-27, when it grew the body heat inputs).
//!
//! Is the player inside the sealed home (oxygenated, the home's own air) or
//! outside (the weather, or vacuum)? And what does the body heat model
//! (`systems::body_heat`) need to know about where they stand: the air's
//! temperature, humidity and pressure, the wind, anything falling on them,
//! and what they are doing. FoodSystem reads the result from the DataStore
//! ("environment_context") to drive oxygen and the core temperature, and the
//! Settings > Gameplay > Body heat mode rides beside it (`body_heat::MODE_KEY`).

use crate::ecs::components::EnvironmentContext;
use crate::engine::state::EngineState;
use crate::systems::body_heat;

/// Earth's pressure scale height, m: the isothermal barometric formula's
/// e-folding height for the lower atmosphere (about 8.4 km).
const EARTH_SCALE_HEIGHT_M: f32 = 8_434.0;
/// Sea-level air pressure, kPa.
const SEA_LEVEL_KPA: f32 = 101.325;

/// Once per frame: publish the survival context and the body heat mode.
pub(crate) fn publish(state: &mut EngineState) {
    let mode = if state.gui_state.settings.body_heat_realistic {
        body_heat::Mode::Realistic
    } else {
        body_heat::Mode::Forgiving
    };
    state.data_store.insert(body_heat::MODE_KEY, mode);

    // Exposed ambient temperature comes from the current weather (winter /
    // storms make the outside deadlier); -40 fallback. PLAYER-LOCAL field, not
    // the body-global `temperature` (review split): hypothermia is about where
    // THIS body stands (latitude, altitude, lunar night), while farming climate
    // and hydrology evaporation keep reading the global reference. See the
    // Weather struct field docs for the full contract. Humidity, wind and
    // what is falling come from the same weather.
    let weather = state
        .data_store
        .get::<std::sync::Mutex<crate::systems::weather::Weather>>("weather")
        .and_then(|m| m.lock().ok())
        .map(|w| {
            (
                w.temperature_at_player,
                w.humidity,
                w.wind_speed,
                body_heat::precipitation(w.condition, w.intensity),
            )
        });
    let (exposed_temp, exposed_rh, exposed_wind, exposed_precip) = weather.unwrap_or((-40.0, 0.0, 0.0, 0.0));
    // Outside the hull: breathable only when standing on a frame-locked body
    // whose air is breathable at this altitude (increment 4). Open space and
    // airless or unbreathable worlds keep the vacuum drain (the existing suit
    // rules). The air pressure: Earth's barometric fall with height where
    // there is air, vacuum where there is none. Other worlds' surface
    // pressures are not in the body data yet, so a world with air takes
    // Earth's; the air supply rules those worlds long before the pressure does.
    let body = state
        .data_store
        .get::<crate::systems::body_environment::BodyEnvironment>("body_environment");
    let outside_breathable = body.map(|e| e.breathable_outside()).unwrap_or(false);
    let outside_kpa = match body {
        Some(e) if e.locked && e.has_atmosphere => SEA_LEVEL_KPA * (-e.altitude_m.max(0.0) / EARTH_SCALE_HEIGHT_M).exp(),
        _ => 0.0,
    };
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
            Some(_) => EnvironmentContext {
                // Outside the hull: unsealed, in the live weather. Breathable
                // ONLY when standing on a body whose open air supports it (Earth
                // below the death zone); space, the Moon, and Mars keep the
                // vacuum oxygen drain (increment 4).
                sealed: false,
                oxygenated: outside_breathable,
                ambient_temp_c: exposed_temp,
                relative_humidity: exposed_rh,
                wind_m_s: exposed_wind,
                precipitation: exposed_precip,
                // Nothing sets shelter yet: the built structures' `shelter`
                // provision will (docs/design/body-heat.md).
                sheltered: false,
                radiant_temp_c: None,
                pressure_kpa: outside_kpa,
                activity_met: activity,
                // The drive does not care which side of the hull you are on:
                // a burn is felt everywhere aboard.
                g_load: felt_g_now,
            },
            // Homestead not generated yet: assume safe.
            None => EnvironmentContext::default(),
        }
    };
    state.data_store.insert("environment_context", env);
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

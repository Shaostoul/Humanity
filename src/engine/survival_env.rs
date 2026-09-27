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
    // Set in the outside branch below; nothing shelters inside the home or in fly mode.
    let mut shelter = uses::ShelterCheck::default();
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
            Some(_) => {
                // Outside the hull: unsealed, in the live weather, under
                // whatever the player has built over themselves (2026-09-27:
                // a roof on three walls keeps the wind and rain off). Only
                // while the player is in the home frame the pieces live in:
                // off the ship the camera does not move with the player, so
                // testing the raw position against home-frame pieces said
                // "Sheltered" wherever one walked (review of the shelter
                // commit; see engine/build_place.rs, which refuses to place
                // off the ship for the same reason).
                if state.aboard_station {
                    shelter = uses::shelter_at(&state.game_world.world, pos - glam::Vec3::Y * state.controller.eye_height());
                }
                let outdoors = Outdoors {
                    temp_c: exposed_temp,
                    relative_humidity: exposed_rh,
                    wind_m_s: exposed_wind,
                    precipitation: exposed_precip,
                    breathable: outside_breathable,
                    pressure_kpa: outside_kpa,
                };
                // The drive does not care which side of the hull you are on:
                // a burn is felt everywhere aboard.
                outside_context(&outdoors, shelter, activity, felt_g_now)
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

/// The weather outside the hull at the player's position.
pub(crate) struct Outdoors {
    pub temp_c: f32,
    pub relative_humidity: f32,
    /// The 10 m wind, m/s.
    pub wind_m_s: f32,
    /// Rain or snow, 0 to 1 (`body_heat::precipitation`).
    pub precipitation: f32,
    /// Breathable ONLY when standing on a body whose open air supports it
    /// (Earth below the death zone); space, the Moon and Mars keep the
    /// vacuum oxygen drain (increment 4).
    pub breathable: bool,
    pub pressure_kpa: f32,
}

/// The survival context outside the hull, under what the player has built
/// over themselves (`uses::shelter_at`). A roof keeps the rain and snow off
/// even with walls missing; a roof on enough walls is `sheltered`, which
/// tells the body heat model the wind does not reach the body either
/// (`body_heat::Exposure::from_context`). The air is the weather's either way.
pub(crate) fn outside_context(o: &Outdoors, shelter: uses::ShelterCheck, activity_met: f32, g_load: f32) -> EnvironmentContext {
    EnvironmentContext {
        sealed: false,
        oxygenated: o.breathable,
        ambient_temp_c: o.temp_c,
        relative_humidity: o.relative_humidity,
        wind_m_s: o.wind_m_s,
        precipitation: if shelter.roofed { 0.0 } else { o.precipitation },
        sheltered: shelter.sheltered(),
        radiant_temp_c: None,
        pressure_kpa: o.pressure_kpa,
        activity_met,
        g_load,
    }
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
    use crate::systems::body_heat::{BodyHeat, Exposure, BASE_OUTFIT_CLO, MET_STANDING};
    use crate::systems::construction::{placement, BlueprintRegistry, Structure};
    use glam::Vec3;

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

        let weather = Outdoors {
            temp_c: 5.0,
            relative_humidity: 0.9,
            wind_m_s: 3.3,
            precipitation: 1.0,
            breathable: true,
            pressure_kpa: SEA_LEVEL_KPA,
        };
        let six_hours = |check: uses::ShelterCheck| {
            let ex = Exposure::from_context(&outside_context(&weather, check, MET_STANDING, 1.0));
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
        let ex = Exposure::from_context(&outside_context(&weather, roof_only, MET_STANDING, 1.0));
        assert_eq!((ex.wind_10m_m_s, ex.precipitation), (3.3, 0.0), "rain off, wind in");
    }
}

//! The survival environment context, published once a frame (moved out of
//! lib.rs 2026-09-27, when it grew the body heat inputs).
//!
//! Is the player inside the sealed home (oxygenated, the home's own air) or
//! outside (the weather, or vacuum)? Outside, are they under a shelter they
//! built (`construction::uses::shelter_at`, 2026-09-27)? And what does the body heat model
//! (`systems::body_heat`) need to know about where they stand: the air's
//! temperature, humidity and pressure, the wind, anything falling on them,
//! what they are doing, and the warmth of any campfire burning near them
//! (BUG-153, 2026-10-05: `warmed_by_fires`). FoodSystem reads the result from the DataStore
//! ("environment_context") to drive oxygen and the core temperature, and the
//! Settings > Gameplay > Body heat mode rides beside it (`body_heat::MODE_KEY`).

use crate::ecs::components::EnvironmentContext;
use crate::engine::state::EngineState;
use crate::systems::body_heat;
use crate::systems::construction::uses;
use crate::systems::weather::Weather;
use glam::Vec3;

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
    /// Where the wind comes FROM, a horizontal unit vector in a build site's
    /// axes (+X east, -Z north, `site::tangent_basis`), zero in calm air:
    /// what `uses::ShelterCheck::wind_share` asks.
    pub upwind: Vec3,
    /// Share of the sky under cloud, 0 to 1 (`Weather::cloud_share`).
    pub cloud: f32,
    /// The sine of the sun's height where the player is (the gameplay sun,
    /// `solar::sun_factor` of the local solar hour: 0 while it is down), set
    /// by `publish`. None until it is, and the surroundings are then the
    /// air's (`body_heat::open_sky_radiant_c` needs the sun).
    pub sun_sin: Option<f32>,
}

impl ExposedAir {
    /// No weather published yet: assume the worst (-40 C, no air).
    const UNKNOWN: ExposedAir =
        ExposedAir { temp_c: -40.0, relative_humidity: 0.0, wind_m_s: 0.0, precipitation: 0.0, pressure_kpa: 0.0, upwind: Vec3::ZERO, cloud: 1.0, sun_sin: None };

    /// The PLAYER-LOCAL fields of the weather, never the body-global
    /// `temperature` (review split): exposure is about where THIS body stands
    /// (latitude, altitude, lunar night), while farming climate and hydrology
    /// evaporation keep reading the global reference. See the Weather struct's
    /// field docs for the full contract. What falls is the phase the air at the
    /// player decides (`Weather::falling_at_player`: a Rain roll in freezing air
    /// lands as snow, and nothing lands where there is no air).
    pub(crate) fn from_weather(w: &Weather) -> Self {
        ExposedAir {
            temp_c: w.temperature_at_player,
            relative_humidity: w.humidity,
            wind_m_s: w.wind_speed_at_player(),
            precipitation: body_heat::precipitation(w.falling_at_player()),
            pressure_kpa: w.pressure_kpa_at_player,
            upwind: upwind_from(w.wind_east_at_player, w.wind_north_at_player),
            cloud: w.cloud_share(),
            sun_sin: None,
        }
    }
}

/// The wind's velocity at the player (east and north components, m/s, the
/// way it blows) turned into where it comes FROM in a build site's axes: east
/// is +X and north is -Z, so the air heads along (east, 0, -north) and comes
/// from the opposite way. Zero in calm air.
pub(crate) fn upwind_from(east: f32, north: f32) -> Vec3 {
    Vec3::new(-east, 0.0, north).normalize_or_zero()
}

/// The context OUTSIDE the hull: unsealed, in the live weather, under what
/// the player has built over themselves (`uses::shelter_at`). Breathable
/// ONLY when standing on a body whose open air supports it (Earth below the
/// death zone); space, the Moon, and Mars keep the vacuum oxygen drain
/// (artificial-planet increment 4). A roof keeps the rain and snow off even
/// with walls missing. The wind that reaches the body under it is the share
/// that comes in through the open sides facing into it
/// (`uses::ShelterCheck::wind_share`, 2026-09-28): a wall on the windward side
/// stops it, so a three-walled shelter with its back to the wind is
/// `sheltered` (still air to the body heat model,
/// `body_heat::Exposure::from_context`) and the same shelter turned into the
/// wind is not. The air is the weather's either way. Pure, so the body heat
/// chain can be tested.
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
        wind_m_s: air.wind_m_s * shelter.wind_share(air.upwind),
        precipitation: if shelter.roofed { 0.0 } else { air.precipitation },
        sheltered: shelter.out_of_the_wind(air.upwind),
        // The open sky, where there is open air to see it through
        // (2026-09-28): the cold of a clear sky and the sun's warmth. A roof
        // overhead radiates at about the air's temperature, hides the sky's
        // coldest part (the zenith) and shades the body, so under one the
        // surroundings are the air's: a shelter's radiant warmth by night and
        // its shade by day. Walls alone leave the sky overhead.
        radiant_temp_c: if breathable && !shelter.roofed {
            air.sun_sin.map(|s| body_heat::open_sky_radiant_c(air.temp_c, air.cloud, s))
        } else {
            None
        },
        pressure_kpa: air.pressure_kpa,
        activity_met,
        // The drive does not care which side of the hull you are on: a burn
        // is felt everywhere aboard.
        g_load,
    }
}

/// The context with the warmth of the fires near the player added
/// (BUG-153, 2026-10-05): `absorbed_w_m2` is what their body takes from every
/// burning fire it has a clear line to (`construction::fires::warmth_at`,
/// which leaves out a fire with a wall, a roof or a shut door between it and
/// the person: radiant heat goes in straight lines, the BUG-153 review), put
/// into the mean radiant temperature (`body_heat::radiant_with_source_c`) on
/// top of the open sky's, or of the air's where a roof hides the sky. Nothing
/// absorbed, nothing changed. Pure, so the chain is tested.
pub(crate) fn warmed_by_fires(mut ctx: EnvironmentContext, absorbed_w_m2: f64) -> EnvironmentContext {
    if absorbed_w_m2 > 0.0 {
        let surroundings = ctx.radiant_temp_c.unwrap_or(ctx.ambient_temp_c);
        ctx.radiant_temp_c = Some(body_heat::radiant_with_source_c(surroundings, absorbed_w_m2));
    }
    ctx
}

/// The heat the player's body takes from the burning fires on the body they
/// stand on, W per square metre of its radiating area (BUG-153). Fires stand
/// only on a planet's ground, so aboard there is none. Their middle is a
/// standing body's (`fires::BODY_MIDDLE_M` above the feet, under the eye at
/// the frame lock's anchor), in the body's frame in f64, which is where every
/// fire's place is taken too (`fires::warmth_at`). A fire with a built piece
/// standing between it and the player gives nothing.
fn fire_warmth(state: &EngineState) -> f64 {
    use crate::systems::construction::fires;
    if state.aboard_station {
        return 0.0;
    }
    let Some(body) = state.frame_lock_body.as_deref() else { return 0.0 };
    let eye = state.frame_lock_anchor;
    let up = eye.normalize_or_zero();
    let middle = eye - up * (crate::surface_walk::EYE_HEIGHT_M - fires::BODY_MIDDLE_M);
    let registry = state.data_store.get::<crate::systems::construction::BlueprintRegistry>("blueprint_registry");
    fires::warmth_at(&state.game_world.world, registry, body, middle, up)
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
    // The air the shelter note is worded for (its upwind as the outside branch set it).
    let mut sheltered_air = exposed;
    let env = if state.controller.fly_mode {
        // Fly mode already suspends vacuum and cold; suspend the burn too, so
        // sightseeing during a 5 g evasion does not quietly kill the operator.
        EnvironmentContext::default()
    } else {
        let air = air_spaces_of(state);
        match whereabouts(air.as_ref(), state.aboard_station, pos) {
            Whereabouts::InsideHome => {
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
                // The air where they stand (2026-10-05, BUG-155): a grow room's
                // own when they stand in one (its heaters' warmth and its damp
                // with it), else the home's own air, warmed by any heater in it.
                // A heater warms the AIR; a fire warms the SURROUNDINGS, from
                // that air (`warmed_by_fires`, BUG-153), so were a fire ever to
                // burn aboard the two would add up, not replace each other
                // (`a_heaters_warm_air_and_a_fires_warmth_add_up`).
                let grow_room = crate::systems::farming::humidity::grow_room_air_at(&state.game_world.world, &state.data_store, pos.to_array());
                let base = EnvironmentContext::default();
                let (air_c, rh, kpa) = indoor_air(home, grow_room, &base);
                EnvironmentContext {
                    oxygenated: breathable,
                    ambient_temp_c: air_c,
                    relative_humidity: rh,
                    pressure_kpa: kpa,
                    activity_met: activity,
                    // A sealed hull stops vacuum, not acceleration.
                    g_load: felt_g_now,
                    ..base
                }
            }
            // The ship's shared spaces (the Commons, First Street, the corridors, a neighbour's
            // plot): the ship's own air, kept by its life support at the standard a home starts at
            // (ship homes increment 4: this was the player's home air everywhere in the box
            // around every room). The ship's air has no model of its own yet, so it is always
            // breathable here.
            Whereabouts::InShip => EnvironmentContext { activity_met: activity, g_load: felt_g_now, ..EnvironmentContext::default() },
            Whereabouts::Outside => {
                // Outside the hull, under whatever the player has built over
                // themselves (2026-09-27: a roof on three walls keeps the wind
                // and rain off), tested in the frame the player is in: the
                // home aboard, or on a planet the build site they stand in,
                // their feet taken from the frame lock's anchor (BUG-102:
                // testing the raw camera position said "Sheltered" wherever
                // one walked, because on a planet the camera does not move).
                let mut air = exposed;
                if let Some(f) = crate::engine::planet_build::player_frame(state) {
                    shelter = uses::shelter_at(&state.game_world.world, f.feet, f.site.as_ref());
                    // The wind's east and north are a PLANET site's axes. The
                    // home frame has no compass (and outside it is vacuum), so
                    // there the wind has no side to come from (review of
                    // 2026-09-28: a roof on the hull read sheltered or not by
                    // a random roll against the home's axes).
                    if f.site.is_none() {
                        air.upwind = Vec3::ZERO;
                    }
                }
                // The sun's height where the player is, from the hour there:
                // the clock the weather's day warmth reads
                // (`weather::local_solar_hour`), on the gameplay sun's arc
                // (`solar::sun_factor`), the one the crops and panels use.
                air.sun_sin = state
                    .data_store
                    .get::<std::sync::Mutex<crate::systems::time::GameTime>>("game_time")
                    .and_then(|m| m.lock().ok().map(|gt| gt.clone()))
                    .map(|gt| {
                        let env = state
                            .data_store
                            .get::<crate::systems::body_environment::BodyEnvironment>("body_environment")
                            .cloned()
                            .unwrap_or_default();
                        let home_lon = crate::systems::time::home_longitude_deg(&state.data_store);
                        crate::systems::solar::sun_factor(crate::systems::weather::local_solar_hour(&gt, &env, home_lon))
                    });
                sheltered_air = air;
                // A campfire's warmth (BUG-153): the fires burning near
                // the player, on top of the open sky's.
                warmed_by_fires(outside_context(air, outside_breathable, shelter, activity, felt_g_now), fire_warmth(state))
            }
            Whereabouts::NoHome => EnvironmentContext::default(),
        }
    };
    state.data_store.insert("environment_context", env);
    // The HUD's Shelter line and the Inventory page's readout.
    state.gui_state.vitals.sheltered = shelter.out_of_the_wind(sheltered_air.upwind);
    state.gui_state.vitals.shelter_note = shelter.note(sheltered_air.upwind);
}

/// Where the player is for the survival context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Whereabouts {
    /// In their own home's sealed air.
    InsideHome,
    /// In the ship's shared air (its zones, corridors and the other plots, increment 4).
    InShip,
    /// Out in the weather (or in space), under whatever they built.
    Outside,
    /// The homestead is not generated yet: assume safe.
    NoHome,
}

/// Whose air the player breathes (src/ship/ship_space.rs), only while ABOARD, in the home frame.
/// On a planet the parked camera's local position can sit inside the home's box by coincidence,
/// because the camera stays put while the ship frame moves under it (the same frame confusion as
/// BUG-102), and that must never read as the home's still, warm air. `air` None: nothing has
/// generated yet.
pub(crate) fn whereabouts(air: Option<&crate::ship::ship_space::AirSpaces>, aboard: bool, pos: glam::Vec3) -> Whereabouts {
    use crate::ship::ship_space::AirAt;
    match air {
        None => Whereabouts::NoHome,
        Some(_) if !aboard => Whereabouts::Outside,
        Some(a) => match a.at(pos) {
            AirAt::OwnHome => Whereabouts::InsideHome,
            AirAt::Ship => Whereabouts::InShip,
            AirAt::Outside => Whereabouts::Outside,
        },
    }
}

/// The air spaces the survival context reads: the ship's (`EngineState::ship_air`) once a ship
/// has assembled; on the legacy layout (no ship) the box around every room is the home, as it
/// was before increment 4; None before anything has generated.
fn air_spaces_of(state: &EngineState) -> Option<crate::ship::ship_space::AirSpaces> {
    match (&state.gui_state.ship_structure, state.homestead_bounds) {
        (Some(_), _) => Some(state.ship_air.clone()),
        (None, Some(b)) => Some(crate::ship::ship_space::AirSpaces { home: Some(b), shared: Vec::new() }),
        (None, None) => None,
    }
}

/// Refresh whose air each place breathes and where aboard ends from the ship as it stands now
/// (ship homes increment 4): called wherever the room boxes are (world_load.rs, home_meshes.rs),
/// so a home moved by a welcome, put away or edited is breathed in where it stands.
pub(crate) fn refresh_ship_spaces(state: &mut EngineState) {
    let ship = state.gui_state.ship_structure.as_ref();
    state.ship_air = ship.map(|s| s.air_spaces()).unwrap_or_default();
    state.aboard_bounds = ship.and_then(|s| s.aboard_bounds());
}

/// The air a body feels inside the home (2026-10-05, BUG-155): temperature
/// (C), relative humidity (0 to 1) and pressure (kPa). A grow room's own air
/// (`farming::humidity::grow_room_air_at`) where the player stands in one, at
/// the home's pressure; else the home's own air (`home_air`); else the
/// context's defaults, before anything is spawned. Pure, so it can be tested.
pub(crate) fn indoor_air(home: Option<(f32, f32, f32)>, grow_room: Option<(f64, f64)>, base: &EnvironmentContext) -> (f32, f32, f32) {
    let kpa = home.map_or(base.pressure_kpa, |h| h.2);
    match (grow_room, home) {
        (Some((t, rh)), _) => (t as f32, (rh as f32).clamp(0.0, 1.0), kpa),
        (None, Some(h)) => (h.0, h.1, kpa),
        (None, None) => (base.ambient_temp_c, base.relative_humidity, kpa),
    }
}

/// THE home's air: temperature (C), relative humidity (0 to 1) and pressure
/// (kPa), from its enclosed space (`atmosphere::HomeAir`). None before the
/// home is spawned. Its temperature carries what heaters have warmed it by
/// (the farming tick writes it, `farming::heat`).
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

    /// THE AIR A BODY FEELS INSIDE THE HOME IS THE ROOM'S IT STANDS IN
    /// (2026-10-05, BUG-155): a grow room's own air where the player stands in
    /// one (a greenhouse a heater has warmed to 22.05 C, at its own 62%), else
    /// the home's own air (whose temperature carries any heater's warmth in it,
    /// farming::heat), else the context's defaults; the pressure is the home's.
    /// Red before this: the greenhouse's air was never read, so the player in a
    /// heated greenhouse felt the house's 19.85 C.
    #[test]
    fn indoors_the_body_feels_the_air_of_the_room_it_stands_in() {
        let base = EnvironmentContext::default();
        let home = Some((19.85_f32, 0.5_f32, 101.3_f32));
        assert_eq!(indoor_air(home, Some((22.05, 0.62)), &base), (22.05, 0.62, 101.3));
        assert_eq!(indoor_air(home, None, &base), (19.85, 0.5, 101.3));
        assert_eq!(indoor_air(None, None, &base), (base.ambient_temp_c, base.relative_humidity, base.pressure_kpa));
    }

    /// A HEATER'S WARM AIR AND A FIRE'S WARMTH ADD UP (BUG-155 with BUG-153,
    /// 2026-10-05). A heater warms the AIR the body is in (`indoor_air`, the
    /// ambient temperature); a fire warms its SURROUNDINGS (`warmed_by_fires`,
    /// the mean radiant temperature), starting from that air when nothing else
    /// sets them. So neither replaces the other: the fire's warmth goes on top
    /// of the air the heater warmed. No fire burns aboard and no heater stands
    /// outside today, so the two never meet in one frame; this holds the
    /// contract for the day they do. In a greenhouse a heater has warmed to
    /// 22.05 C with a campfire's 130 W/m2 (about what a body takes 1.5 m from
    /// one), the body keeps the heater's air, its surroundings are warmer than
    /// with the fire alone, and after an hour its skin is warmer than with
    /// either alone. Red with `warmed_by_fires` setting the radiant temperature
    /// from the rooms' fixed 21 C instead of the context's air: the heater's
    /// warmth was lost under the fire's.
    #[test]
    fn a_heaters_warm_air_and_a_fires_warmth_add_up() {
        let base = EnvironmentContext::default();
        let home = Some((19.85_f32, 0.5_f32, 101.3_f32));
        let indoors = |grow_room: Option<(f64, f64)>| {
            let (air_c, rh, kpa) = indoor_air(home, grow_room, &base);
            EnvironmentContext { ambient_temp_c: air_c, relative_humidity: rh, pressure_kpa: kpa, ..base }
        };
        let flux = 130.0;
        let neither = indoors(None);
        let heater = indoors(Some((22.05, 0.5)));
        let fire = warmed_by_fires(indoors(None), flux);
        let both = warmed_by_fires(indoors(Some((22.05, 0.5))), flux);
        assert_eq!(both.ambient_temp_c, 22.05, "the fire leaves the heater's air alone");
        let radiant = |c: &EnvironmentContext| c.radiant_temp_c.unwrap_or(c.ambient_temp_c);
        assert_eq!(radiant(&both), body_heat::radiant_with_source_c(22.05, flux), "the fire's warmth on top of the heater's air");
        assert!(radiant(&both) > radiant(&fire) && radiant(&both) > radiant(&heater), "surroundings {} with both, {} fire, {} heater", radiant(&both), radiant(&fire), radiant(&heater));
        let skin_after_an_hour = |c: &EnvironmentContext| {
            let ex = body_heat::Exposure::from_context(c);
            let mut body = body_heat::BodyHeat::new(body_heat::CORE_NEUTRAL_C);
            for _ in 0..60 {
                body.step(&ex, body_heat::BASE_OUTFIT_CLO, body_heat::MET_STANDING, 60.0);
            }
            body.skin_c
        };
        let (b, f, h, n) = (skin_after_an_hour(&both), skin_after_an_hour(&fire), skin_after_an_hour(&heater), skin_after_an_hour(&neither));
        assert!(b > f && b > h && h > n, "skin after an hour: both {b:.3}, fire {f:.3}, heater {h:.3}, neither {n:.3}");
    }

    /// INSIDE THE HOME REQUIRES ABOARD (the review's missing test). The same
    /// camera position inside the home's box is the home's air aboard and
    /// the weather on a planet, where the parked camera sits there by
    /// coincidence; outside the box it is the weather either way; with no
    /// home generated, the safe default. Red check, run: dropping the
    /// `aboard` condition from `whereabouts` puts the planet player in the
    /// home's air and the second assertion fails.
    #[test]
    fn inside_the_home_requires_being_aboard() {
        use glam::Vec3;
        let air = crate::ship::ship_space::AirSpaces { home: Some((Vec3::new(-10.0, 0.0, -10.0), Vec3::new(10.0, 20.0, 10.0))), shared: Vec::new() };
        let home = Some(&air);
        let parked = Vec3::new(0.0, 5.0, 0.0);
        assert_eq!(whereabouts(home, true, parked), Whereabouts::InsideHome);
        assert_eq!(whereabouts(home, false, parked), Whereabouts::Outside, "on a planet the parked camera is not in the home");
        assert_eq!(whereabouts(home, true, Vec3::new(0.0, 50.0, 0.0)), Whereabouts::Outside, "on the hull, outside the box");
        assert_eq!(whereabouts(home, true, Vec3::new(10.0, 20.0, 10.0)), Whereabouts::InsideHome, "the box's own corner is inside");
        assert_eq!(whereabouts(None, false, parked), Whereabouts::NoHome);
    }

    /// INCREMENT 4: on the shipped ship the player breathes their own home's air only in their
    /// own home; the Commons is the ship's air (sealed and breathable whatever the home's air
    /// does), and over the roof is outside. Seen red 2026-10-04 with every pressurized space
    /// counted as the home's (`AirSpaces::at`), as the box around every room was before
    /// increment 4: "the Commons is the ship's air, not InsideHome".
    #[test]
    fn the_commons_breathes_the_ships_air_not_the_homes() {
        use glam::Vec3;
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let ship = crate::ship::ship_structure::ShipStructure::load_and_assemble_shipped(&data, Some("p1")).expect("the shipped ship");
        let air = ship.air_spaces();
        let w = whereabouts(Some(&air), true, Vec3::new(80.0, 1.7, 60.0));
        assert_eq!(w, Whereabouts::InShip, "the Commons is the ship's air, not {w:?}");
        assert_eq!(whereabouts(Some(&air), true, Vec3::new(26.0, 1.7, 40.0)), Whereabouts::InsideHome);
        assert_eq!(whereabouts(Some(&air), true, Vec3::new(26.0, 30.0, 40.0)), Whereabouts::Outside);
    }

    /// THE TOP OF THE HOMESTEAD'S OWN ELEVATOR BREATHES THE HOME'S AIR (ship homes increment 4
    /// review, P2), and so do the top of its ladder, the top step of its stairs, and a jump from
    /// the elevator's top. The home's box used to stop at its roof (3 m for the homestead), so the
    /// eye of anyone who rode its elevator up a storey (to 4.7 m) was in no air at all: outside,
    /// in vacuum, hypoxia after 20 s. Before increment 4 every room's box reached the Commons' 8 m.
    /// High over the roof is still outside.
    ///
    /// Seen red 2026-10-04 on the code before the fix: "the elevator's car at the top (elevator-1),
    /// the eye at [31, 4.7, 26], breathes Outside".
    #[test]
    fn the_top_of_the_homesteads_own_elevator_breathes_the_homes_air() {
        use glam::Vec3;
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let ship = crate::ship::ship_structure::ShipStructure::load_and_assemble_shipped(&data, Some("p1")).expect("the shipped ship");
        let air = ship.air_spaces();
        let eye = crate::surface_walk::EYE_HEIGHT_M as f32;
        let jump_rise = crate::renderer::camera::JUMP_SPEED_MPS.powi(2) / (2.0 * 9.81);
        let places = [
            ("the elevator's car at the top (elevator-1)", Vec3::new(31.0, 3.0 + eye, 26.0)),
            ("the ladder's top (ladder-1)", Vec3::new(11.0, 3.0 + eye + 0.2, 32.0)),
            ("the stairs' top step (stairs-1)", Vec3::new(34.0, 3.0 + eye, 5.7)),
            ("a jump from the elevator's top", Vec3::new(31.0, 3.0 + eye + jump_rise, 26.0)),
        ];
        for (what, p) in places {
            let w = whereabouts(Some(&air), true, p);
            assert_eq!(w, Whereabouts::InsideHome, "{what}, the eye at {p}, breathes {w:?}");
        }
        assert_eq!(whereabouts(Some(&air), true, Vec3::new(26.0, 30.0, 40.0)), Whereabouts::Outside, "high over the roof");
    }

    /// The headroom a home's air keeps over the highest thing in it (src/ship/ship_space.rs
    /// `HEADROOM_M`) covers a person standing there and jumping, by the camera's own numbers:
    /// the eye height and the jump's rise at the homestead's gravity (data/game.csv
    /// `gravity_m_s2`).
    #[test]
    fn a_homes_headroom_covers_a_jump_at_its_gravity() {
        let csv = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("game.csv")).expect("data/game.csv");
        let g: f32 = csv
            .lines()
            .find_map(|l| l.strip_prefix("gravity_m_s2,"))
            .and_then(|rest| rest.split(',').next())
            .and_then(|v| v.trim().parse().ok())
            .expect("gravity_m_s2 in data/game.csv");
        let needed = crate::surface_walk::EYE_HEIGHT_M as f32 + crate::renderer::camera::JUMP_SPEED_MPS.powi(2) / (2.0 * g);
        let headroom = crate::ship::ship_space::HEADROOM_M;
        assert!(headroom >= needed, "a home's headroom of {headroom} m is under a standing jump's {needed:.2} m at {g} m/s2");
    }

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

    /// THE BODY FEELS THE PHASE THE WEATHER DECIDED (2026-09-27). One Rain
    /// condition, pinned from the F11 panel at 0.9, at 70 N inland on the
    /// northern winter solstice and at the equator the same day. The whole
    /// chain runs (WeatherSystem's export, `ExposedAir::from_weather`, the body
    /// heat input): the arctic body gets snow, which wets at a third, and the
    /// tropical body gets the rain at its full 0.9, and the input is exactly
    /// what the export's own `falling_at_player` says. Red check, run:
    /// `from_weather` reading the condition as before (Rain means rain) puts
    /// 0.9 on the arctic body and the first precipitation assertion fails.
    #[test]
    fn the_body_heat_input_gets_the_phase_the_weather_decided() {
        use crate::systems::time::{GameTime, DEFAULT_DAYS_PER_YEAR, EARTH_DAY_S};
        use crate::systems::weather::{ManualWeather, WeatherCondition, WeatherControl};
        let mut data = DataStore::new();
        data.insert("weather", std::sync::Mutex::new(Weather::default()));
        let mut clock = GameTime {
            elapsed_seconds: 0.75 * f64::from(DEFAULT_DAYS_PER_YEAR) * EARTH_DAY_S,
            ..Default::default()
        };
        clock.recompute_derived();
        data.insert("game_time", std::sync::Mutex::new(clock));
        data.insert(
            "weather_control",
            std::sync::Mutex::new(WeatherControl {
                manual: Some(ManualWeather { condition: WeatherCondition::Rain, intensity: 0.9, wind_speed: 2.0 }),
                retrigger: true,
            }),
        );
        let at = |lat: f64| {
            let la = lat.to_radians();
            BodyEnvironment {
                locked: true,
                latitude_deg: lat as f32,
                altitude_m: 0.0,
                up_dir: glam::DVec3::new(la.cos(), la.sin(), 0.0),
                land_fraction: 1.0,
                ..Default::default()
            }
        };
        let mut world = hecs::World::new();
        let mut sys = WeatherSystem::new();
        let mut read = |lat: f64, sys: &mut WeatherSystem| {
            data.insert("body_environment", at(lat));
            sys.tick(&mut world, 0.0, &data);
            data.get::<std::sync::Mutex<Weather>>("weather").unwrap().lock().unwrap().clone()
        };
        let arctic = read(70.0, &mut sys);
        let tropic = read(0.0, &mut sys);
        assert_eq!((arctic.condition, tropic.condition), (WeatherCondition::Rain, WeatherCondition::Rain));
        assert!(arctic.temperature_at_player < -5.0, "Layer 1 freezes 70 N in winter: {}", arctic.temperature_at_player);
        assert!(tropic.temperature_at_player > 15.0, "and not the equator: {}", tropic.temperature_at_player);
        let (a, t) = (ExposedAir::from_weather(&arctic), ExposedAir::from_weather(&tropic));
        assert!((a.precipitation - 0.3).abs() < 0.01, "snow on the arctic body, wetting at a third: {a:?}");
        assert!((t.precipitation - 0.9).abs() < 0.01, "rain on the tropical body: {t:?}");
        assert_eq!(a.precipitation, body_heat::precipitation(arctic.falling_at_player()));
        assert_eq!(arctic.condition_at_player(), WeatherCondition::Snow, "the HUD says snow");
        assert_eq!(tropic.condition_at_player(), WeatherCondition::Rain);
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
            let tf = placement::placement_pose(bp, Vec3::new(x, 0.0, z), turns, &world, &reg, None);
            world.spawn((tf, Structure { blueprint_id: id.into(), health: bp.health, max_health: bp.health, provides: bp.provides.clone(), uid: 0 }));
        }
        let under = uses::shelter_at(&world, Vec3::new(0.0, 0.0, 0.5), None);
        let open = uses::shelter_at(&world, Vec3::new(8.0, 0.0, 0.0), None);
        assert!(under.sheltered() && !open.sheltered(), "{under:?} {open:?}");

        let weather = ExposedAir {
            temp_c: 5.0,
            relative_humidity: 0.9,
            wind_m_s: 3.3,
            precipitation: 1.0,
            pressure_kpa: SEA_LEVEL_KPA,
            // From the north: the shelter's walls stand north, west and east.
            upwind: Vec3::NEG_Z,
            cloud: 1.0,
            sun_sin: None,
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
        // West and east walls only: the north wind comes straight in.
        let roof_only = uses::ShelterCheck { roofed: true, walls: 0b0011 };
        let ex = Exposure::from_context(&outside_context(weather, true, roof_only, MET_STANDING, 1.0));
        assert_eq!((ex.wind_10m_m_s, ex.precipitation), (3.3, 0.0), "rain off, wind in");
    }

    /// Where the wind comes FROM, in a build site's axes: blowing east it
    /// comes from the west (-X); blowing north, from the south (+Z, since -Z is
    /// north there). Red check, run: flipping the sign of east in upwind_from
    /// fails the first assertion.
    /// A ROOF KEEPS THE NIGHT SKY OFF (2026-09-28). A clear, calm, dry 10 C
    /// night, standing in the everyday outfit: in the open the body radiates
    /// to a sky about 10 degrees colder than the air; under a roof (no walls
    /// needed: there is no wind) the surroundings are the air's. The core
    /// barely moves either way (36.70 C under the roof, 36.67 C in the open:
    /// the body defends it), so the difference shows where it is paid for:
    /// the skin ends about 1.9 C warmer under the roof (25.5 C against
    /// 23.6 C) and the body shivers about 40 percent less (16 W/m2 against
    /// 26). Red check, run: leaving
    /// `radiant_temp_c` at None (the air everywhere, as before) fails the
    /// first assertion.
    #[test]
    fn a_roof_keeps_the_night_sky_off_the_body() {
        use crate::systems::body_heat::{BodyHeat, Exposure, BASE_OUTFIT_CLO, MET_STANDING};
        let night = ExposedAir {
            temp_c: 10.0,
            relative_humidity: 0.7,
            wind_m_s: 0.0,
            precipitation: 0.0,
            pressure_kpa: SEA_LEVEL_KPA,
            upwind: Vec3::ZERO,
            cloud: 0.0,
            sun_sin: Some(0.0),
        };
        let six_hours = |check: uses::ShelterCheck| {
            let ex = Exposure::from_context(&outside_context(night, true, check, MET_STANDING, 1.0));
            let mut body = BodyHeat::new(37.0);
            for _ in 0..360 {
                body.step(&ex, BASE_OUTFIT_CLO, MET_STANDING, 60.0);
            }
            (ex, body)
        };
        let (ex_open, open) = six_hours(uses::ShelterCheck::default());
        let (ex_roof, roofed) = six_hours(uses::ShelterCheck { roofed: true, walls: 0 });
        assert!(ex_open.radiant_c < 1.0, "the open sees the clear sky: {} C", ex_open.radiant_c);
        assert_eq!(ex_roof.radiant_c, 10.0, "under the roof, the air's");
        assert!(
            roofed.skin_c > open.skin_c + 1.5 && roofed.shiver_w_m2 < 0.75 * open.shiver_w_m2,
            "after 6 h, roofed skin {:.2} C shiver {:.1} W/m2, open skin {:.2} C shiver {:.1}",
            roofed.skin_c,
            roofed.shiver_w_m2,
            open.skin_c,
            open.shiver_w_m2
        );
        assert!(roofed.core_c >= open.core_c, "and the core no colder");
        // Airless or unbreathable (outside the hull, the Moon): no sky term.
        let ex_space = Exposure::from_context(&outside_context(night, false, uses::ShelterCheck::default(), MET_STANDING, 1.0));
        assert_eq!(ex_space.radiant_c, 10.0);
    }

    /// A ROOF IS SHADE AT NOON (2026-09-28). A clear, dry 30 C noon with a
    /// light 1 m/s breeze, standing in the everyday outfit for two hours: in
    /// the open the sun lifts the mean radiant temperature far above the air
    /// (SolarCal); under a roof, walls or not, it is the air's. After two
    /// hours the core is about a quarter of a degree warmer in the sun and
    /// the skin about 1.5 C warmer. Red check,
    /// run: leaving the sun out of `open_sky_radiant_c` fails the first
    /// assertion.
    #[test]
    fn a_roof_is_shade_at_noon() {
        use crate::systems::body_heat::{BodyHeat, Exposure, BASE_OUTFIT_CLO, MET_STANDING};
        let noon = ExposedAir {
            temp_c: 30.0,
            relative_humidity: 0.4,
            wind_m_s: 1.0,
            precipitation: 0.0,
            pressure_kpa: SEA_LEVEL_KPA,
            upwind: Vec3::ZERO,
            cloud: 0.0,
            sun_sin: Some(1.0),
        };
        let two_hours = |check: uses::ShelterCheck| {
            let ex = Exposure::from_context(&outside_context(noon, true, check, MET_STANDING, 1.0));
            let mut body = BodyHeat::new(37.0);
            for _ in 0..120 {
                body.step(&ex, BASE_OUTFIT_CLO, MET_STANDING, 60.0);
            }
            (ex, body)
        };
        let (ex_sun, sun) = two_hours(uses::ShelterCheck::default());
        let (ex_shade, shade) = two_hours(uses::ShelterCheck { roofed: true, walls: 0 });
        // Measured 2026-09-28: core 37.19 C in the sun, 36.93 C in the shade;
        // skin 36.23 C and 34.78 C.
        const SHADE_CORE_MARGIN_C: f64 = 0.2;
        const SHADE_SKIN_MARGIN_C: f64 = 1.2;
        assert!(ex_sun.radiant_c > 50.0, "the noon sun in the open: {} C", ex_sun.radiant_c);
        assert_eq!(ex_shade.radiant_c, 30.0, "in the shade, the air's");
        assert!(
            sun.core_c > shade.core_c + SHADE_CORE_MARGIN_C && sun.skin_c > shade.skin_c + SHADE_SKIN_MARGIN_C,
            "after 2 h, sun core {:.3} C skin {:.2} C, shade core {:.3} C skin {:.2} C",
            sun.core_c,
            sun.skin_c,
            shade.core_c,
            shade.skin_c
        );
    }

    // ── BUG-153: a campfire's warmth reaches the body ─────────────────

    /// A clear, calm 0 C night with the sun down: the cold case a campfire is for.
    const CLEAR_FREEZING_NIGHT: ExposedAir = ExposedAir {
        temp_c: 0.0,
        relative_humidity: 0.7,
        wind_m_s: 0.0,
        precipitation: 0.0,
        pressure_kpa: SEA_LEVEL_KPA,
        upwind: Vec3::ZERO,
        cloud: 0.0,
        sun_sin: Some(0.0),
    };

    /// A finished campfire at the origin of a build site on Earth's ground,
    /// with `fuel_s` of burning left, as the ConstructionSystem leaves one.
    fn campfire_site(fuel_s: f32) -> (hecs::World, crate::systems::construction::BlueprintRegistry, crate::systems::construction::PlanetSite) {
        use crate::ecs::components::Transform;
        use crate::systems::construction::{fires::FireFuel, BlueprintRegistry, PlanetSite, Structure};
        let reg = BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap();
        let site = PlanetSite { body: "earth".into(), origin: glam::DVec3::new(0.0, 6_371_000.0, 0.0) };
        let bp = reg.get("campfire").expect("campfire in basic.ron");
        let mut world = hecs::World::new();
        world.spawn((
            Transform { position: Vec3::ZERO, rotation: glam::Quat::IDENTITY, scale: Vec3::from_array(bp.size) },
            Structure { blueprint_id: "campfire".into(), health: bp.health, max_health: bp.health, provides: bp.provides.clone(), uid: 1 },
            site.clone(),
            FireFuel { seconds_left: fuel_s },
        ));
        (world, reg, site)
    }

    /// What a person standing `x` metres from the fire's centre is exposed to
    /// on the clear freezing night: their middle in the site, the warmth they
    /// take from the fires (`fires::warmth_at`), the open context and the
    /// fires' warmth on it (`warmed_by_fires`), as the body model reads it.
    fn by_the_fire(world: &hecs::World, reg: &crate::systems::construction::BlueprintRegistry, site: &crate::systems::construction::PlanetSite, x: f32) -> (f64, body_heat::Exposure) {
        use crate::systems::construction::fires;
        let middle = site.to_body(Vec3::new(x, fires::BODY_MIDDLE_M as f32, 0.0));
        let warmth = fires::warmth_at(world, Some(reg), "earth", middle, site.origin.normalize());
        let ctx = outside_context(CLEAR_FREEZING_NIGHT, true, uses::ShelterCheck::default(), body_heat::MET_STANDING, 1.0);
        (warmth, body_heat::Exposure::from_context(&warmed_by_fires(ctx, warmth)))
    }

    /// A body in the everyday outfit standing for `hours` in `ex`.
    fn standing_for(ex: &body_heat::Exposure, hours: u32) -> body_heat::BodyHeat {
        let mut body = body_heat::BodyHeat::new(body_heat::CORE_NEUTRAL_C);
        for _ in 0..hours * 60 {
            body.step(ex, body_heat::BASE_OUTFIT_CLO, body_heat::MET_STANDING, 60.0);
        }
        body
    }

    /// BUG-153. A CAMPFIRE ON A COLD NIGHT KEEPS A BODY WARMER THAN ONE 20 M
    /// AWAY. A clear, calm 0 C night, standing in the everyday outfit: 1.5 m
    /// from a burning campfire's centre the surroundings the body feels are
    /// about 16 C (the open sky's are -11 C), and after an hour the core is
    /// warmer, the skin much warmer and the body shivers less than 20 m away,
    /// where the fire moves the surroundings by under half a degree. Half a
    /// metre from the ring (1 m from its centre) the body keeps a normal core
    /// through the whole night without shivering enough to show, while 20 m
    /// away it shivers hard all night: the fire keeps a person in ordinary
    /// clothes warm on a freezing night, from close enough. The whole chain: a
    /// finished campfire in a build site, `fires::warmth_at` at the person's
    /// middle, `outside_context`, `warmed_by_fires`, `Exposure::from_context`,
    /// the body model. Seen red 2026-10-05 with `warmed_by_fires` passing the
    /// context through unchanged, which is the code before the fix (no fire
    /// term anywhere): the 1.5 m and 20 m bodies were the same.
    #[test]
    fn a_campfire_by_a_cold_night_keeps_a_body_warmer_than_one_20_m_away() {
        let (world, reg, site) = campfire_site(2.0 * 3600.0);
        let open = body_heat::Exposure::from_context(&outside_context(
            CLEAR_FREEZING_NIGHT,
            true,
            uses::ShelterCheck::default(),
            body_heat::MET_STANDING,
            1.0,
        ));
        let (_, near) = by_the_fire(&world, &reg, &site, 1.5);
        let (_, close) = by_the_fire(&world, &reg, &site, 1.0);
        let (_, far) = by_the_fire(&world, &reg, &site, 20.0);
        assert!(open.radiant_c < -10.0, "the clear night sky: {} C", open.radiant_c);
        assert!(near.radiant_c > 12.0, "1.5 m from the fire: {} C", near.radiant_c);
        assert!((far.radiant_c - open.radiant_c).abs() < 0.5, "20 m away: {} C against the open {} C", far.radiant_c, open.radiant_c);

        let (near_1h, far_1h) = (standing_for(&near, 1), standing_for(&far, 1));
        assert!(
            near_1h.core_c > far_1h.core_c + 0.05,
            "after an hour, core {:.3} C at 1.5 m and {:.3} C at 20 m",
            near_1h.core_c,
            far_1h.core_c
        );
        assert!(near_1h.skin_c > far_1h.skin_c + 2.0, "skin {:.2} C at 1.5 m, {:.2} C at 20 m", near_1h.skin_c, far_1h.skin_c);
        assert!(near_1h.shiver_w_m2 < far_1h.shiver_w_m2, "shivering {:.1} W/m2 at 1.5 m, {:.1} at 20 m", near_1h.shiver_w_m2, far_1h.shiver_w_m2);

        let (close_night, far_night) = (standing_for(&close, 8), standing_for(&far, 8));
        assert!(
            !close_night.is_shivering() && close_night.core_c > 36.6,
            "8 h half a metre from the ring: core {:.2} C, shivering {:.1} W/m2",
            close_night.core_c,
            close_night.shiver_w_m2
        );
        assert!(far_night.is_shivering(), "8 h at 20 m: shivering {:.1} W/m2", far_night.shiver_w_m2);
    }

    /// BUG-153. AN OUT CAMPFIRE GIVES NO HEAT. The same freezing night beside
    /// the same campfire with its fuel burnt out: the fire gives nothing, the
    /// surroundings are exactly the open sky's, and the body after an hour is
    /// exactly the one with no fire at all. Seen red 2026-10-05 with
    /// `fires::warmth_at` counting every fire, burning or not.
    #[test]
    fn an_out_campfire_gives_no_heat() {
        let (world, reg, site) = campfire_site(0.0);
        let (warmth, ex) = by_the_fire(&world, &reg, &site, 1.5);
        assert_eq!(warmth, 0.0, "an out fire radiates nothing");
        let open = body_heat::Exposure::from_context(&outside_context(
            CLEAR_FREEZING_NIGHT,
            true,
            uses::ShelterCheck::default(),
            body_heat::MET_STANDING,
            1.0,
        ));
        assert_eq!(ex, open, "beside an out fire, the open night");
        assert_eq!(standing_for(&ex, 1), standing_for(&open, 1));
    }

    #[test]
    fn upwind_is_where_the_wind_comes_from_in_a_sites_axes() {
        assert_eq!(upwind_from(3.0, 0.0), Vec3::NEG_X);
        assert_eq!(upwind_from(0.0, 2.0), Vec3::Z);
        assert_eq!(upwind_from(0.0, 0.0), Vec3::ZERO, "calm");
    }
}

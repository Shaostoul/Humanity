//! Space heaters and the air they warm, through the real FarmingSystem tick
//! (2026-10-05, BUG-155). heat.rs is the model and data/garden/humidity.ron
//! (THE HEAT) its numbers; these prove a heater's watts reach the air it
//! stands in, settle where a hand calculation puts them, reach what reads
//! that air, and that the Greenhouse Construction quest promises no more than
//! that. Each was seen red first. With `heat::heaters` made to find no heater,
//! which is the code before this fix in effect (no heater had any effect), the
//! five that run a heater failed, every one on its room or the home's air
//! staying at its own temperature; the doc comment on each says what else was
//! broken to see it red.

use std::sync::Mutex;

use super::gardening_tests::make_store;
use super::humidity::{self, AirMap, GrowRoom, HumidityData, HUMIDITY_RON};
use super::lighting::GrowPlot;
use super::*;
use crate::ecs::components::{PowerConsumer, RoomAir, SoilMemory, SpaceHeater, Transform};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::atmosphere::{EnclosedSpace, HomeAir};

fn air() -> HumidityData {
    HumidityData::parse(HUMIDITY_RON).expect("the shipped humidity.ron parses")
}

/// The channels the farming tick reads, the shipped air data, no pests, and
/// the given room boxes and grow plots.
fn store(rooms: Vec<GrowRoom>, plots: Vec<GrowPlot>) -> DataStore {
    let mut data = make_store();
    data.insert("garden_pest_severity", Mutex::new(0.0_f32));
    data.insert("player_notices", Mutex::new(Vec::<String>::new()));
    data.insert("hand_water_draw_l", Mutex::new(0.0_f32));
    data.insert("pest_control_request", Mutex::new(Option::<(String, String)>::None));
    data.insert(humidity::DATA_KEY, air());
    data.insert(humidity::ROOMS_KEY, Mutex::new(rooms));
    data.insert("grow_plots", plots);
    data
}

/// A grow room `id`, `w` x `h` x `w` metres from the origin, with one grow bed
/// ("bed_a") standing in it: the bed is what makes it a room with its own air.
fn one_room(id: &str, w: f32, h: f32) -> DataStore {
    let room = GrowRoom { id: id.into(), name: "Greenhouse".into(), min: [0.0, 0.0, 0.0], max: [w, h, w], ..Default::default() };
    let bed = GrowPlot { id: "bed_a".into(), pos: [w / 2.0, 0.0, w / 2.0], ..Default::default() };
    store(vec![room], vec![bed])
}

/// A space heater at `at`, `watts` (a resistance heater: its heat is its
/// draw), its thermostat at `setpoint_c`, with or without power.
fn heater(world: &mut hecs::World, at: [f32; 3], watts: f32, setpoint_c: f32, powered: bool) -> hecs::Entity {
    world.spawn((
        SpaceHeater { heat_w: watts, watts, setpoint_c },
        Transform { position: glam::Vec3::from_array(at), ..Default::default() },
        PowerConsumer { draw_watts: watts, priority: 4, enabled: powered },
    ))
}

/// `ticks` farming ticks of `dt` real seconds (the store's clock runs 72 game
/// seconds a real second, so 5 s is a tenth of a game hour).
fn run(sys: &mut FarmingSystem, world: &mut hecs::World, data: &DataStore, ticks: usize, dt: f32) {
    for _ in 0..ticks {
        sys.tick(world, dt, data);
    }
}

fn memory(world: &hecs::World) -> SoilMemory {
    world.query::<&SoilMemory>().iter().next().map(|(_, m)| m.clone()).unwrap_or_default()
}

fn room(world: &hecs::World, id: &str) -> RoomAir {
    memory(world).rooms.get(id).copied().unwrap_or_default()
}

fn draw(world: &hecs::World, e: hecs::Entity) -> f32 {
    world.get::<&PowerConsumer>(e).unwrap().draw_watts
}

/// A cubic metre of air's heat capacity, J/K, at `t_c`, BY HAND: the density
/// FAO-56's way, P / (R T) with P 101.3 kPa and R 287 J/(kg K) (humidity.ron),
/// times its cp, 1,013 J/(kg K).
fn air_j_m3_k(t_c: f64) -> f64 {
    101_300.0 / (287.0 * (t_c + 273.15)) * 1013.0
}

/// A POWERED HEATER WARMS ITS ROOM TO THE STEADY STATE A HAND CALCULATION GIVES.
/// A 1,500 W heater, its thermostat at 40 C so it never stops, stands in a
/// 300 m3 grow room (10 x 3 x 10 m) with no home air around it (the air around
/// stays at its own temperature). By hand (humidity.ron, THE HEAT): the walls
/// and ceiling, 2 x (10 + 10) x 3 + 10 x 10 = 220 m2 at 6.24 W/(m2 K), lose
/// 1,372.8 W a degree, and the room's leakage, half its 300 m3 an hour at
/// 1,215.5 J/(m3 K), 50.65 W a degree: 1,423.45 W/K, so it settles 1,500 /
/// 1,423.45 = 1.054 C over, at 22.05 C, with the heater flat out at 1,500 W.
/// What reads the room's air sees that warmth: its vapour reads drier at
/// 22.05 C than at 21.
///
/// Red with no heater found (the code before this fix in effect): the room
/// stayed at warmed 0. Also red with the room's leakage dropped from its
/// conductance (`n` passed as 0 in step_rooms): it settled at 1.093 C.
#[test]
fn a_powered_heater_warms_its_room_to_the_steady_state_worked_by_hand() {
    let data = one_room("room-a", 10.0, 3.0);
    let mut world = hecs::World::new();
    let e = heater(&mut world, [5.0, 0.5, 5.0], 1500.0, 40.0, true);
    let mut sys = FarmingSystem::new();
    // 3 game hours: the air's time constant is about 4 game minutes.
    run(&mut sys, &mut world, &data, 30, 5.0);
    let g = 6.24 * (2.0 * (10.0 + 10.0) * 3.0 + 10.0 * 10.0) + air_j_m3_k(21.0) * 0.5 * 300.0 / 3600.0;
    let want = 1500.0 / g;
    assert!((g - 1423.45).abs() < 0.01, "by hand: {g} W/K");
    let r = room(&world, "room-a");
    assert!((r.warmed_k - want).abs() < 1e-6, "the room warmed {} K, by hand {want} K", r.warmed_k);
    assert!((want - 1.0538).abs() < 0.0001, "the 1.05 C humidity.ron quotes: {want}");
    assert_eq!(r.heater, 1.0, "never at its 40 C, so flat out");
    assert!((draw(&world, e) - 1500.0).abs() < 1e-3, "it draws its 1,500 W: {}", draw(&world, e));
    // What reads the room's air reads it at 22.05 C.
    let d = air();
    let map = AirMap::new(&world, &data, &d);
    let rh = map.rh_for(&d, "bed_a", &memory(&world).rooms);
    assert!((rh - d.rh_of(r.vapour_g_m3, 21.0 + want)).abs() < 1e-12, "{rh}");
    assert!(rh < 0.95 * d.rh_of(r.vapour_g_m3, 21.0), "the warmed air reads drier: {rh}");
    assert!((humidity::room_temp_at(&d, r.warmed_k) - 22.054).abs() < 0.001);
}

/// ONE SPACE HEATER BARELY WARMS A GREENHOUSE THE SIZE OF THE FAMILY HOME'S
/// (45 x 3 x 22 m, 2,970 m3, data/homes/homestead.ron `room-greenhouse`), on
/// its own leakage with nothing else in it: by hand its walls and ceiling,
/// 2 x (45 + 22) x 3 + 45 x 22 = 1,392 m2, lose 8,686 W a degree and its
/// leakage 501 W, so the heater, flat out and never near 24 C, adds 0.163 C.
/// The figure humidity.ron and the heating guide quote: a heater has to match
/// the heat a space loses. Red with no heater found (the code before this fix
/// in effect): 0.
#[test]
fn one_heater_barely_warms_a_greenhouse_the_size_of_the_family_homes() {
    let greenhouse = GrowRoom { id: "room-greenhouse".into(), name: "Greenhouse".into(), min: [0.0, 0.0, 0.0], max: [45.0, 3.0, 22.0], ..Default::default() };
    let bed = GrowPlot { id: "bed_a".into(), pos: [20.0, 0.0, 10.0], ..Default::default() };
    let data = store(vec![greenhouse], vec![bed]);
    let mut world = hecs::World::new();
    let e = heater(&mut world, [20.0, 0.5, 10.0], 1500.0, 24.0, true);
    let mut sys = FarmingSystem::new();
    run(&mut sys, &mut world, &data, 30, 5.0);
    let g = 6.24 * (2.0 * (45.0 + 22.0) * 3.0 + 45.0 * 22.0) + air_j_m3_k(21.0) * 0.5 * 2970.0 / 3600.0;
    let r = room(&world, "room-greenhouse");
    assert!((r.warmed_k - 1500.0 / g).abs() < 1e-6, "warmed {} K, by hand {}", r.warmed_k, 1500.0 / g);
    assert!((r.warmed_k - 0.163).abs() < 0.0005, "{}", r.warmed_k);
    assert_eq!(r.heater, 1.0, "never near 24 C, so flat out");
    assert!((draw(&world, e) - 1500.0).abs() < 1e-3);
}

/// AN UNPOWERED HEATER WARMS NOTHING, and still asks for its watts. The same
/// room and heater with its PowerConsumer off (the electrical sim shed it, or it
/// has no cable): the room stays exactly at its own temperature, its humidity
/// reads exactly as it did before heaters existed, and the heater's draw is
/// what it would take with power (its thermostat would run it flat out), so
/// the electrical sim sees the request and can give it its power back.
///
/// Red with `h.powered` read as true in `heat::heaters` (the room warmed
/// 1.054 C with no power).
#[test]
fn an_unpowered_heater_warms_nothing_and_asks_for_its_power() {
    let data = one_room("room-a", 10.0, 3.0);
    let mut world = hecs::World::new();
    let e = heater(&mut world, [5.0, 0.5, 5.0], 1500.0, 40.0, false);
    let mut sys = FarmingSystem::new();
    run(&mut sys, &mut world, &data, 30, 5.0);
    let r = room(&world, "room-a");
    assert_eq!(r.warmed_k, 0.0, "no power, no heat");
    assert_eq!(r.heater, 0.0);
    assert!((draw(&world, e) - 1500.0).abs() < 1e-3, "it asks for its 1,500 W: {}", draw(&world, e));
    let d = air();
    let rh = AirMap::new(&world, &data, &d).rh_for(&d, "bed_a", &memory(&world).rooms);
    assert_eq!(rh, d.rh_of(r.vapour_g_m3, d.room_temp_c).min(1.0), "read at the rooms' own 21 C");
}

/// THE THERMOSTAT HOLDS ITS SETPOINT, running for the share of the time the
/// room's loss at it takes. A 1,500 W heater set to 24 C in a 2 x 2 x 2 m grow
/// room: by hand its walls and ceiling, 2 x (2 + 2) x 2 + 2 x 2 = 20 m2, lose
/// 124.8 W a degree and its leakage (4 m3 an hour) 1.35, 126.15 W/K, so flat
/// out it would run 11.9 C over its 21 C. Held at 24 C, 3 C over, it loses
/// 378.5 W, so the heater runs 25.2% of the time and draws 378.5 W.
///
/// Red with the thermostat ignored (the duty fixed at 1 in `heat::step`): the
/// room ran to 11.89 C over.
#[test]
fn the_thermostat_holds_its_setpoint() {
    let data = one_room("room-a", 2.0, 2.0);
    let mut world = hecs::World::new();
    let e = heater(&mut world, [1.0, 0.5, 1.0], 1500.0, 24.0, true);
    let mut sys = FarmingSystem::new();
    run(&mut sys, &mut world, &data, 30, 5.0);
    let g = 6.24 * (2.0 * (2.0 + 2.0) * 2.0 + 2.0 * 2.0) + air_j_m3_k(21.0) * 0.5 * 8.0 / 3600.0;
    let r = room(&world, "room-a");
    assert!((r.warmed_k - 3.0).abs() < 1e-6, "held at 24 C: warmed {} K", r.warmed_k);
    assert!((r.heater - 3.0 * g / 1500.0).abs() < 1e-6, "on {} of the time, by hand {}", r.heater, 3.0 * g / 1500.0);
    assert!((r.heater - 0.2523).abs() < 0.0001, "{}", r.heater);
    assert!((f64::from(draw(&world, e)) - 3.0 * g).abs() < 0.01, "draws {} W, by hand {} W", draw(&world, e), 3.0 * g);
    // Flat out it would have gone to 1,500 / 126.15 = 11.9 C over.
    assert!((1500.0 / g - 11.89).abs() < 0.01);
}

/// A HEATER IN A ROOM WITH NO GROW MACHINE WARMS THE HOME'S OWN AIR, the air
/// the body heat model reads inside the home. A sealed 1,000 m3 home and a
/// 1,500 W heater standing in no grow room: by hand, the home's own air is one
/// storey 3 m high on a square floor (humidity.ron, home_storey_m), its roof
/// 333.3 m2 and its walls 4 x 18.26 x 3 = 219.1 m2, 552.4 m2 at 6.24 W/(m2 K):
/// 3,447 W/K, so it settles 0.435 C over its own. The farming tick writes
/// that into the home's air space, which `engine::survival_env` reads for a
/// player inside the home: 293.435 K.
///
/// Red with the space's temperature not written (the farming tick's write
/// removed): the space stayed at 293.0 K though the home's air was warmed.
#[test]
fn a_heater_outside_the_grow_rooms_warms_the_home_air_the_body_reads() {
    let data = store(Vec::new(), Vec::new());
    let mut world = hecs::World::new();
    world.spawn((HomeAir { metabolic_kcal_per_day: 0.0, ..Default::default() }, EnclosedSpace::new_sealed(1000.0)));
    let e = heater(&mut world, [50.0, 0.5, 50.0], 1500.0, 40.0, true);
    let mut sys = FarmingSystem::new();
    run(&mut sys, &mut world, &data, 30, 5.0);
    let floor = 1000.0 / 3.0;
    let g = 6.24 * (floor + 4.0 * f64::sqrt(floor) * 3.0);
    let want = 1500.0 / g;
    let home = memory(&world).home_air;
    assert!((home.warmed_k - want).abs() < 1e-6, "the home's air warmed {} K, by hand {want} K", home.warmed_k);
    assert!((want - 0.4351).abs() < 0.0001, "{want}");
    assert_eq!(home.heater, 1.0);
    assert!((draw(&world, e) - 1500.0).abs() < 1e-3);
    let space = world.query::<(&HomeAir, &EnclosedSpace)>().iter().next().map(|(_, (h, s))| (h.own_temp_k, s.atmosphere.temperature_k)).unwrap();
    assert_eq!(space.0, 293.0, "its own temperature, recorded");
    assert!((f64::from(space.1) - (293.0 + want)).abs() < 1e-4, "the space reads {} K", space.1);
    // And the AirMap the life support reads its gases at sees the same.
    let d = air();
    let map = AirMap::new(&world, &data, &d);
    assert!((map.home_temp_c - (19.85 + want)).abs() < 1e-4, "{}", map.home_temp_c);
}

/// A GROW ROOM'S HEAT GOES ON INTO THE HOME'S AIR, AND NONE IS LOST. The heater
/// in the 300 m3 grow room of a sealed 3,300 m3 home: everything it puts out
/// leaves the home through the home's own walls and roof in the end, so, by
/// hand, the home's own air (3,000 m3, the grow room is its own) settles 1,500 /
/// 8,607.9 = 0.174 C over, and the grow room the same 1.054 C over that as
/// alone: 1.228 C. And with the heater switched off the home cools back to
/// exactly its own temperature: nothing builds up.
///
/// Red with a room's passed heat dropped instead of added to the home's
/// (nothing added to `home_heat_in_j`): the home stayed at 0.
#[test]
fn a_grow_rooms_heat_reaches_the_home_air_and_none_is_lost() {
    let data = one_room("room-a", 10.0, 3.0);
    let mut world = hecs::World::new();
    world.spawn((HomeAir { metabolic_kcal_per_day: 0.0, ..Default::default() }, EnclosedSpace::new_sealed(3300.0)));
    let e = heater(&mut world, [5.0, 0.5, 5.0], 1500.0, 40.0, true);
    let mut sys = FarmingSystem::new();
    run(&mut sys, &mut world, &data, 60, 5.0);
    let floor = 3000.0 / 3.0;
    let g_home = 6.24 * (floor + 4.0 * f64::sqrt(floor) * 3.0);
    let g_room = 6.24 * 220.0 + air_j_m3_k(21.0) * 0.5 * 300.0 / 3600.0;
    let (home_k, room_k) = (memory(&world).home_air.warmed_k, room(&world, "room-a").warmed_k);
    assert!((home_k - 1500.0 / g_home).abs() < 1e-6, "the home warmed {home_k} K, by hand {}", 1500.0 / g_home);
    assert!((room_k - (1500.0 / g_home + 1500.0 / g_room)).abs() < 1e-6, "the room warmed {room_k} K");
    assert!((room_k - 1.228).abs() < 0.001, "{room_k}");
    world.get::<&mut PowerConsumer>(e).unwrap().enabled = false;
    run(&mut sys, &mut world, &data, 60, 5.0);
    let (home_k, room_k) = (memory(&world).home_air.warmed_k, room(&world, "room-a").warmed_k);
    assert!(home_k < 1e-9 && room_k < 1e-9, "cooled back: home {home_k}, room {room_k}");
}

/// THE STEP IS EXACT: over any slice, the heat the heaters put in is the heat
/// the air kept, plus what it gave the air around it, plus what its coils took
/// (`heat::step` on `life_support::relax`), whatever the slice's length; and
/// one long slice and many short ones agree on where a heater that never
/// reaches its setpoint ends. Red with the heat passed reckoned against 0
/// instead of the air around it (`to_sink(k, 0.0, ..)`): the joules no longer
/// add up.
#[test]
fn the_heat_step_keeps_every_joule() {
    use super::heat::{step, AirHeat};
    let a = AirHeat {
        warmed_k: 0.3,
        capacity_j_k: 364_650.0,
        around_w_k: 1423.45,
        around_k: 0.1,
        coil_w_k: 300.0,
        in_w: 50.0,
        heat_w: 1500.0,
        heat_all_w: 1500.0,
        set_k: Some(100.0),
    };
    for hours in [0.001, 0.05, 0.25, 3.0] {
        let w = step(&a, hours);
        let put_in = (1500.0 * w.duty + 50.0) * hours * 3600.0;
        let kept = a.capacity_j_k * (w.warmed_k - a.warmed_k);
        assert!((put_in - kept - w.passed_j - w.coil_j).abs() < 1e-6 * put_in, "{hours} h: in {put_in}, kept {kept}, passed {}, coils {}", w.passed_j, w.coil_j);
    }
    let one = step(&a, 1.0).warmed_k;
    let mut many = AirHeat { ..a };
    for _ in 0..100 {
        many.warmed_k = step(&many, 0.01).warmed_k;
    }
    assert!((one - many.warmed_k).abs() < 1e-9, "{one} against {}", many.warmed_k);
}

/// THE TIME AWAY CHARGES A HEATER WHAT ITS THERMOSTAT RAN IT AT (the second
/// seam review of BUG-155, 2026-10-05). While the player is away the power
/// ledger charges the home's machines the Usage meter's day averages
/// (`crafting::away::day_power_balance`), and the meter cannot see the air a
/// heater stands in, so it charged every heater its full 1,500 W, 36 kWh a
/// day, even one its thermostat runs an eighth of the time.
///
/// Two of the shipped catalog's heaters, spawned the way the engine spawns
/// them: one in a mushroom rack's fruiting tent (the shipped tent of
/// data/garden/grow_media.ron, 1.3 x 1.9 x 0.7 m around 22.7 kg of
/// substrate, standing in no room, so the air around it stays at its own
/// temperature), one in a grow room the size of the family home's
/// greenhouse. By hand the tent's walls and top, 2 x (1.3 + 0.7) x 1.9 + 1.3
/// x 0.7 = 8.51 m2, lose 53.1 W a degree, and its fresh air
/// (`tent_fresh_air_m3_h`, 22.6 m3 an hour) about 7.6 W, so its heater holds
/// it 3 C over its 21 C on about 182 W, 12% of the time; the greenhouse's
/// never reaches 24 C and runs flat out. The save keeps both draws (through
/// JSON), and the time away `resume_home` hands the machines, with the
/// shipped family home and the two heaters placed in it, has a day's balance
/// better than the meter's by the tent heater's unused 1,318 W in both life
/// support modes: the flat-out heater is charged its full 1,500 W, the tent's
/// its 182.
///
/// Seen red with the saved draws ignored, the code before this fix in effect:
///   Station-supplied: the time away charges the heaters 0.0 W less than their
///   full draw, not the tent heater's unused 1317.8 W
/// and with the save not keeping them (`extract_world_save` without
/// `heater_draw_w`): "the save has heater_tent's draw: {}".
#[cfg(feature = "native")]
#[test]
fn the_time_away_charges_a_heater_what_its_thermostat_ran_it_at() {
    use crate::machines::{MachineHome, MachineInstance};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let tent = crate::systems::grow_machines::load_grow_media(&root.join("data"))
        .into_iter()
        .find_map(|m| m.enclosure)
        .expect("the shipped fruiting tent");
    assert_eq!((tent.size, tent.substrate_kg), ((1.3, 1.9, 0.7), 22.7), "the tent this test's hand numbers are for");
    let greenhouse = GrowRoom { id: "room-greenhouse".into(), name: "Greenhouse".into(), min: [0.0, 0.0, 0.0], max: [45.0, 3.0, 22.0], ..Default::default() };
    let bed = GrowPlot { id: "bed_a".into(), pos: [20.0, 0.0, 10.0], ..Default::default() };
    let rack = GrowPlot { id: "rack_a".into(), pos: [60.0, 0.0, 5.0], enclosure: Some(tent.clone()), ..Default::default() };
    let data = store(vec![greenhouse], vec![bed, rack]);

    // The shipped family home, with the two heaters placed in it and spawned.
    let mut home = MachineHome::load(&root.join("data/machines/home.ron")).expect("home.ron parses");
    let def = home.catalog.get("heater").expect("home.ron catalogs the heater").clone();
    let mut world = hecs::World::new();
    let empty = std::collections::HashMap::new();
    for (id, at) in [("heater_tent", (60.0, 0.5, 5.0)), ("heater_greenhouse", (20.0, 0.5, 10.0))] {
        let inst = MachineInstance {
            id: id.into(),
            machine: "heater".into(),
            room: String::new(),
            offset: at,
            rotation: 0.0,
            zone: "home".into(),
            screen_source: None,
        };
        crate::engine::home_spawn::spawn_home_machine_entity(&mut world, &inst, &def, &empty, &empty, None, None);
        home.instances.push(inst);
    }
    let mut sys = FarmingSystem::new();
    run(&mut sys, &mut world, &data, 30, 5.0);

    // The tent's heater by hand; the greenhouse's flat out.
    let d = air();
    let g_tent = 6.24 * (2.0 * (1.3 + 0.7) * 1.9 + 1.3 * 0.7) + air_j_m3_k(21.0) * d.tent_fresh_air_m3_h(22.7) / 3600.0;
    let tent_w = 3.0 * g_tent;
    assert!((tent_w - 182.2).abs() < 0.1, "by hand, the tent's heater draws {tent_w} W");
    let saved = crate::save_load::extract_world_save(&world);
    let saved: crate::persistence::WorldSave = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
    let draw = |id: &str| f64::from(*saved.heater_draw_w.get(id).unwrap_or_else(|| panic!("the save has {id}'s draw: {:?}", saved.heater_draw_w)));
    assert!((draw("heater_tent") - tent_w).abs() < 0.01, "the tent's heater saved at {} W, by hand {tent_w} W", draw("heater_tent"));
    assert!((draw("heater_greenhouse") - 1500.0).abs() < 1e-3, "the greenhouse's flat out: {} W", draw("heater_greenhouse"));

    // An hour later the game resumes the home: the time away it hands the
    // machines charges each heater its saved draw.
    let mut save = saved;
    save.timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() - 3600;
    let mut away_data = DataStore::new();
    away_data.insert("auto_mine_order", Mutex::new(Option::<(String, Vec<(String, u32)>)>::None));
    crate::systems::crafting::register(&mut away_data);
    crate::systems::livestock::register(&mut away_data);
    let mut fresh = hecs::World::new();
    crate::save_load::apply_save_to_world(&mut fresh, &save);
    crate::save_load::resume_home(&mut fresh, &away_data, &save, true, 1.0, Some(&home));
    let work = crate::systems::crafting::away::take(&away_data).expect("an hour away handed to the machines");
    let meter = crate::systems::crafting::away::day_power_balance(&home, &Default::default());
    for (mode, (away, full)) in ["Station-supplied", "Realistic"].iter().zip(work.power_balance_w.iter().zip(meter.iter())) {
        let better = f64::from(away - full);
        assert!(
            (better - (1500.0 - tent_w)).abs() < 0.05,
            "{mode}: the time away charges the heaters {better:.1} W less than their full draw, not the tent heater's unused {:.1} W",
            1500.0 - tent_w
        );
    }
}

/// A well-watered `plant` on unit `slot` of `area`, at its first stage: the
/// Garden panel only has a line for an area with a living crop.
fn crop(data: &DataStore, plant: &str, area: &str, slot: u32) -> crate::ecs::components::CropInstance {
    let stage = data.get::<PlantRegistry>("plant_registry").unwrap().get(plant).unwrap().first_stage().to_string();
    crate::ecs::components::CropInstance {
        crop_def_id: plant.to_string(),
        growth_stage: stage,
        planted_at: 0.0,
        water_level: 1.0,
        health: 100.0,
        tower_id: Some(area.to_string()),
        tower_slot: Some(slot),
        health_seconds: 0.0,
        growing_seconds: 0.0,
    }
}

/// THE GARDEN PANEL SAYS WHAT EACH HEATER IS DOING. The heater holding the
/// small room at 24 C: "heater on 25% of the time, 378 W, holding the air at
/// 24 °C"; the same heater without power: "heater off (no power), the air at
/// 21.0 °C" once the room has cooled back; and a heater in the home's own air,
/// which cannot reach its setpoint there, on a line of the home's own: "Home
/// air: heater flat out, 1,500 W, the air at 20.3 °C, short of the 24 °C it is
/// set to". Red with `heater_part` left out of the room's line (no heater
/// words at all).
#[test]
fn the_garden_panel_says_what_each_heater_is_doing() {
    let data = one_room("room-a", 2.0, 2.0);
    let mut world = hecs::World::new();
    let e = heater(&mut world, [1.0, 0.5, 1.0], 1500.0, 24.0, true);
    let mut sys = FarmingSystem::new();
    run(&mut sys, &mut world, &data, 30, 5.0);
    world.spawn((crop(&data, "lettuce", "bed_a", 0),));
    let line = |world: &hecs::World| humidity::GuiView::new(world, &data).areas.iter().find(|(a, _, _)| a == "bed_a").map(|l| l.1.clone()).unwrap();
    let held = line(&world);
    assert!(held.contains("heater on 25% of the time, 378 W, holding the air at 24 °C"), "{held}");
    world.get::<&mut PowerConsumer>(e).unwrap().enabled = false;
    run(&mut sys, &mut world, &data, 30, 5.0);
    let off = line(&world);
    assert!(off.contains("heater off (no power), the air at 21.0 °C"), "{off}");

    let home_data = store(Vec::new(), Vec::new());
    let mut home_world = hecs::World::new();
    home_world.spawn((HomeAir { metabolic_kcal_per_day: 0.0, ..Default::default() }, EnclosedSpace::new_sealed(1000.0)));
    heater(&mut home_world, [50.0, 0.5, 50.0], 1500.0, 24.0, true);
    let mut home_sys = FarmingSystem::new();
    run(&mut home_sys, &mut home_world, &home_data, 30, 5.0);
    let view = humidity::GuiView::new(&home_world, &home_data);
    assert!(
        view.home.iter().any(|(l, _)| l == "Home air: heater flat out, 1,500 W, the air at 20.3 °C, short of the 24 °C it is set to"),
        "{:?}",
        view.home
    );
}

/// THE QUEST'S PROMISE IS WHAT THE HEATER DOES (BUG-155). The Greenhouse
/// Construction quest asks for the Build Heater recipe; the recipe makes a
/// `heater_0`; that item is what places the catalog's `heater` machine in both
/// shipped homes (`machines::placement_item_id`); the machine is a resistance
/// heater whose heat is its draw, with a thermostat set above the grow rooms'
/// own temperature; spawned the way the engine spawns it, standing in a grow
/// room with power, it warms that room's air; and the step says so, in words
/// that fit the HUD's quest line.
///
/// Red on the data before this fix, each part alone: the step's old words
/// ("Build a heater for temperature regulation", promised from a heater that
/// did nothing), and home.ron with no `heater` in its catalog (the crafted
/// item placed nothing). Also red with the catalog's thermostat at the rooms'
/// own 21 C, where the heater would never run in a grow room.
#[cfg(feature = "native")]
#[test]
fn the_greenhouse_quests_heater_warms_a_grow_rooms_air() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    // The quest's step.
    let quests: Vec<crate::systems::quests::QuestDef> =
        ron::from_str(&std::fs::read_to_string(root.join("data/quests/farming.ron")).unwrap()).expect("farming.ron parses");
    let q = quests.iter().find(|q| q.id == "farming_greenhouse").expect("the Greenhouse Construction quest");
    let step = q
        .steps
        .iter()
        .find(|s| matches!(&s.objective, crate::systems::quests::QuestObjective::Craft { recipe_id, .. } if recipe_id == "build_heater"))
        .expect("a step that crafts the heater");
    assert_eq!(step.description, "Build a space heater to warm a grow room's air");
    // The recipe makes the item the machine is placed from.
    let recipes = std::fs::read_to_string(root.join("data/recipes.csv")).unwrap();
    let row = recipes.lines().find(|l| l.starts_with("build_heater,")).expect("the Build Heater recipe");
    assert_eq!(row.split(',').nth(4), Some("heater_0:1"), "{row}");
    assert_eq!(crate::machines::placement_item_id("heater"), "heater_0");
    let d = air();
    for file in ["home.ron", "home_solo.ron"] {
        let home = crate::machines::MachineHome::load(&root.join("data/machines").join(file)).unwrap_or_else(|| panic!("{file} parses"));
        let def = home.catalog.get("heater").unwrap_or_else(|| panic!("{file} catalogs the heater"));
        let Some(crate::machines::MachinePower::Consumer { watts, .. }) = def.power else { panic!("{file}: the heater is a Consumer") };
        assert_eq!(def.heats_w, watts, "{file}: a resistance heater's heat is its draw");
        assert!(f64::from(def.heat_setpoint_c) > d.room_temp_c, "{file}: set above the rooms' own {} C", d.room_temp_c);
        // Nothing else in the catalog heats.
        for (id, other) in &home.catalog {
            assert_eq!(other.heats_w > 0.0, id == "heater", "{file}: `{id}` heats only if it is the heater");
        }
        // Spawned as the engine spawns it, in a powered grow room.
        let data = one_room("room-a", 2.0, 2.0);
        let mut world = hecs::World::new();
        let inst = crate::machines::MachineInstance {
            id: "heater_t".into(),
            machine: "heater".into(),
            room: "room-a".into(),
            offset: (1.0, 0.0, 1.0),
            rotation: 0.0,
            zone: "home".into(),
            screen_source: None,
        };
        let empty = std::collections::HashMap::new();
        crate::engine::home_spawn::spawn_home_machine_entity(&mut world, &inst, def, &empty, &empty, None, None);
        let mut sys = FarmingSystem::new();
        run(&mut sys, &mut world, &data, 30, 5.0);
        let r = room(&world, "room-a");
        assert!(
            (humidity::room_temp_at(&d, r.warmed_k) - f64::from(def.heat_setpoint_c)).abs() < 1e-6,
            "{file}: the room is held at the heater's {} C, it is at {}",
            def.heat_setpoint_c,
            humidity::room_temp_at(&d, r.warmed_k)
        );
    }
}

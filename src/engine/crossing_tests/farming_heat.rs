//! From systems/farming/heat_tests.rs: space heaters, stoves and a heat pump spawned the way the
//! engine spawns them (`engine::home_spawn`), and the machines' day through a save and load
//! (`save_load`). Helpers they share with the tests that stayed are that file's.

use std::sync::Mutex;
use crate::ecs::components::PowerConsumer;
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::atmosphere::{EnclosedSpace, HomeAir};
use crate::systems::farming::heat_tests::*;
use crate::systems::farming::humidity::{self, AirMap, GrowRoom};
use crate::systems::farming::lighting::GrowPlot;
use crate::systems::farming::*;

/// THE TIME AWAY CHARGES EACH MACHINE THE DAY IT REALLY DREW (the second seam
/// review of BUG-155, 2026-10-05; the day record, 2026-10-22). While the
/// player is away the power ledger charges the home's machines the Usage
/// meter's day (`crafting::away::day_power_balance`), and a heater's data
/// says only its full 1,500 W, 36 kWh a day, even for one its thermostat runs
/// an eighth of the time.
///
/// Two of the shipped catalog's heaters, spawned the way the engine spawns
/// them: one in a mushroom rack's fruiting tent (the shipped tent of
/// data/garden/grow_media.ron, 1.3 x 1.9 x 0.7 m around 22.7 kg of
/// substrate, standing in no room, so the air around it stays at its own
/// temperature), one in a greenhouse-sized grow room, 44.5 x 21 m and 6 m
/// tall. By hand the tent's walls and top, 2 x (1.3 + 0.7) x 1.9 + 1.3 x 0.7
/// = 8.51 m2, lose 53.1 W a degree, and its fresh air (`tent_fresh_air_m3_h`,
/// 22.6 m3 an hour) about 7.6 W, so its heater holds it 3 C over its 21 C on
/// about 182 W, 12% of the time; the greenhouse's never reaches 24 C and runs
/// flat out. They run a game day and two hours with the machines' power tick
/// recording what each drew (`machines::day_energy`); the save keeps that day
/// (through the save file), it comes back with the home, and the time away
/// `resume_home` hands the machines, with the shipped family home and the two
/// heaters placed in it, has a day's balance better than the meter's estimate
/// by the tent heater's unused 1,318 W in both life support modes: the
/// flat-out heater is charged its full 1,500 W, the tent's its 182.
///
/// Seen red (2026-10-23) with `MachineHome::day_draw` reading only the data:
///   "Station-supplied: the time away charges the heaters 0.0 W less than their
///   full draw, not the tent heater's unused 1317.8 W"
/// and with `save_home_at` not writing the record: "the save has heater_tent's
/// day: None". Until 2026-10-22 the save kept each heater's draw at the moment
/// it was written (`heater_draw_w`); the record replaced it with a day of it.
#[cfg(feature = "native")]
#[test]
fn the_time_away_charges_each_machine_the_day_it_drew() {
    use crate::machines::MachineInstance;
    use crate::machines::day_energy;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let tent = crate::systems::grow_machines::load_grow_media(&root.join("data"))
        .into_iter()
        .find_map(|m| m.enclosure)
        .expect("the shipped fruiting tent");
    assert_eq!((tent.size, tent.substrate_kg), ((1.3, 1.9, 0.7), 22.7), "the tent this test's hand numbers are for");
    let greenhouse = GrowRoom { id: "room-greenhouse".into(), name: "Greenhouse".into(), min: [0.0, 0.0, 0.0], max: [44.5, 6.0, 21.0], ..Default::default() };
    let bed = GrowPlot { id: "bed_a".into(), pos: [20.0, 0.0, 10.0], ..Default::default() };
    let rack = GrowPlot { id: "rack_a".into(), pos: [60.0, 0.0, 5.0], enclosure: Some(tent.clone()), ..Default::default() };
    let mut data = store(vec![greenhouse], vec![bed, rack]);
    day_energy::register(&mut data);
    data.insert("power_status", Mutex::new(crate::systems::electrical::PowerStatus::default()));

    // The shipped family home, with the two heaters placed in it and spawned.
    let mut home = crate::systems::grow_machines::load_home(&root.join("data/machines/home.ron")).expect("home.ron parses");
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
    // A supply on their island (0, the spawn's with no wiring) that carries both.
    world.spawn((
        crate::ecs::components::PowerGenerator { output_watts: 10_000.0, fuel_per_second: 0.0, active: true },
        crate::ecs::components::PowerCircuit { island: 0 },
    ));
    let mut sys = FarmingSystem::new();
    let mut power = crate::systems::electrical::ElectricalSystem::new(std::path::Path::new("data"));
    // 26 game hours, a tick a tenth of one (5 s at the store's 72x): the day the
    // record keeps starts after the heaters have settled.
    for _ in 0..260 {
        power.tick(&mut world, 5.0, &data);
        sys.tick(&mut world, 5.0, &data);
    }

    // The tent's heater by hand; the greenhouse's flat out.
    let d = air();
    let g_tent = 6.24 * (2.0 * (1.3 + 0.7) * 1.9 + 1.3 * 0.7) + air_j_m3_k(21.0) * d.tent_fresh_air_m3_h(22.7) / 3600.0;
    let tent_w = 3.0 * g_tent;
    assert!((tent_w - 182.2).abs() < 0.1, "by hand, the tent's heater draws {tent_w} W");
    // Saved through the save file, as the game saves.
    let dir = std::env::temp_dir().join(format!("hos_day_energy_away_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("saves").join("offline_home.json");
    crate::save_load::save_home_at(&path, &world, &[], &data, true, None);
    let saved = crate::persistence::load_world(&path).expect("the save reads back");
    let _ = std::fs::remove_dir_all(&dir);
    let day = |id: &str| saved.machine_day_energy.day_of(id, "heater").map(|d| d.average_w());
    let tent_day = day("heater_tent").unwrap_or_else(|| panic!("the save has heater_tent's day: {:?}", day("heater_tent")));
    assert!((tent_day - tent_w).abs() < 0.01, "the tent's heater drew {tent_day} W over its day, by hand {tent_w} W");
    let greenhouse_day = day("heater_greenhouse").expect("the save has heater_greenhouse's day");
    assert!((greenhouse_day - 1500.0).abs() < 1e-3, "the greenhouse's flat out: {greenhouse_day} W");

    // An hour later the game resumes the home: the record comes back with it,
    // and the time away it hands the machines charges each heater its day.
    let mut save = saved;
    save.timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() - 3600;
    let mut away_data = DataStore::new();
    away_data.insert("auto_mine_order", Mutex::new(Option::<(String, Vec<(String, u32)>)>::None));
    crate::systems::crafting::register(&mut away_data);
    crate::systems::livestock::register(&mut away_data);
    day_energy::register(&mut away_data);
    let mut fresh = hecs::World::new();
    crate::save_load::apply_save_to_world(&mut fresh, &save);
    crate::save_load::resume_home(&mut fresh, &away_data, &save, true, 1.0, Some(&home));
    assert_eq!(day_energy::snapshot(&away_data).machines, save.machine_day_energy.machines, "the record comes back with the home");
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
        let home = crate::systems::grow_machines::load_home(&root.join("data/machines").join(file)).unwrap_or_else(|| panic!("{file} parses"));
        let def = home.catalog.get("heater").unwrap_or_else(|| panic!("{file} catalogs the heater"));
        let Some(crate::machines::MachinePower::Consumer { watts, .. }) = def.power else { panic!("{file}: the heater is a Consumer") };
        assert_eq!(def.heats_w, watts, "{file}: a resistance heater's heat is its draw");
        assert!(f64::from(def.heat_setpoint_c) > d.room_temp_c, "{file}: set above the rooms' own {} C", d.room_temp_c);
        // Nothing else in the catalog heats but the heat pump and the wood
        // stove (2026-10-20), each the kind its data says it is.
        for (id, other) in &home.catalog {
            let heater = ["heater", "heat_pump", "wood_stove"].contains(&id.as_str());
            assert_eq!(other.heats_w > 0.0, heater, "{file}: `{id}` heats only if it is one of the three heaters");
            assert_eq!(!other.heat_cop.is_empty(), id == "heat_pump", "{file}: `{id}`: only the heat pump has a COP");
            assert_eq!(other.burns_fuel.is_some(), id == "wood_stove", "{file}: `{id}`: only the wood stove burns fuel");
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

/// The shipped catalog `file`'s machine `id`.
fn shipped(file: &str, id: &str) -> crate::machines::MachineDef {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let home = crate::systems::grow_machines::load_home(&root.join("data/machines").join(file)).unwrap_or_else(|| panic!("{file} parses"));
    home.catalog.get(id).unwrap_or_else(|| panic!("{file} catalogs `{id}`")).clone()
}

/// The shipped container registry, which gives the stove its box.
fn shipped_containers() -> crate::systems::inventory::containers::ContainerRegistry {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let read = |rel: &str| std::fs::read(root.join(rel)).unwrap_or_else(|_| panic!("data/{rel}"));
    crate::systems::inventory::containers::ContainerRegistry::from_bytes(&read("containers/types.csv"), &read("containers/content_classes.ron"))
        .expect("container types")
        .with_contact_rules(&read("containers/materials.csv"), &read("containers/content_traits.ron"), &read("food/item_profiles.ron"))
        .expect("contact rules")
}

/// home.ron's `machine` placed at `at` and spawned the way the engine spawns
/// it, a stove with its box.
#[cfg(feature = "native")]
fn spawn_shipped(world: &mut hecs::World, machine: &str, id: &str, at: [f32; 3]) -> hecs::Entity {
    spawn_def(world, &shipped("home.ron", machine), machine, id, at)
}

/// `def` (catalogued as `machine`) placed at `at` and spawned the way the
/// engine spawns it, a stove with its box: for a shipped machine whose data a
/// test changes first.
#[cfg(feature = "native")]
fn spawn_def(world: &mut hecs::World, def: &crate::machines::MachineDef, machine: &str, id: &str, at: [f32; 3]) -> hecs::Entity {
    let inst = crate::machines::MachineInstance {
        id: id.into(),
        machine: machine.into(),
        room: String::new(),
        offset: (at[0], at[1], at[2]),
        rotation: 0.0,
        zone: "home".into(),
        screen_source: None,
    };
    let empty = std::collections::HashMap::new();
    let reg = shipped_containers();
    crate::engine::home_spawn::spawn_home_machine_entity(world, &inst, def, &empty, &empty, None, Some(&reg));
    world
        .query::<&crate::ecs::components::MachineInstanceId>()
        .iter()
        .find(|(_, m)| m.0 == id)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("{machine} spawned"))
}

/// Put `n` Wood Logs into the stove's box through the box's own Store rule.
#[cfg(feature = "native")]
fn load_logs(world: &mut hecs::World, data: &DataStore, stove: hecs::Entity, n: u32) {
    use crate::systems::inventory::containers::{Container, StoreOutcome};
    let items = data.get::<crate::systems::inventory::ItemRegistry>("item_registry").unwrap();
    let reg = shipped_containers();
    let mut c = world.get::<&mut Container>(stove).expect("the stove has its box");
    let out = reg.try_store(&mut c, "wood_log_0", items.class_for("wood_log_0"), items.volume_for("wood_log_0"), n);
    assert_eq!(out, StoreOutcome::Stored { quantity: n }, "the box takes {n} Wood Logs");
}

/// The wood still in a stove's box, kg: its logs less what has burned of the
/// one at the front.
fn wood_kg(world: &hecs::World, stove: hecs::Entity) -> f64 {
    let c = world.get::<&crate::systems::inventory::containers::Container>(stove).unwrap();
    let s = world.get::<&crate::ecs::components::FuelStove>(stove).unwrap();
    let log = if c.current_content_item.as_deref() == Some("wood_log_0") { 8.0 } else { 0.0 };
    f64::from(c.current_qty) * log - s.burned_kg
}

/// The family home's greenhouse, the room the game publishes for it (44.5 x 6
/// x 21 m, 5,607 m3, open to the 6 m roof), with one grow bed: by hand (the
/// second test above) it loses 1,720.5 m2 x 6.24 W/(m2 K) through its walls
/// and ceiling and half an air change an hour of its lowest 3 m storey,
/// 11,209 W a degree, so even the wood stove's 13,040 W never brings it to
/// 24 C and it burns flat out there. (A 45 x 3 x 22 m room, 9,187.5 W a
/// degree, until 2026-10-21.)
fn greenhouse_store() -> DataStore {
    let greenhouse = GrowRoom { id: "room-greenhouse".into(), name: "Greenhouse".into(), min: [0.0, 0.0, 0.0], max: [44.5, 6.0, 21.0], ..Default::default() };
    let bed = GrowPlot { id: "bed_a".into(), pos: [20.0, 0.0, 10.0], ..Default::default() };
    store(vec![greenhouse], vec![bed])
}

fn greenhouse_w_k() -> f64 {
    6.24 * (2.0 * (44.5 + 21.0) * 6.0 + 44.5 * 21.0) + air_j_m3_k(21.0) * 0.25 * 5607.0 / 3600.0
}

/// A HEAT PUMP PUTS ITS COP TIMES ITS DRAW INTO THE AIR, AND IS CHARGED ONLY
/// ITS DRAW (2026-10-20). home.ron's heat pump (the Mitsubishi MSZ-FS09NA:
/// 620 W, its sheet's COP 4.54 at 47 F, 8.3 C), spawned the way the engine
/// spawns it in the 300 m3 grow room of the first test, with the station's
/// air around the home at its own 19.85 C: warmer than the sheet's warmest
/// point, so its COP is 4.54. By hand its heat is 620 x 4.54 = 2,814.8 W, and
/// the room settles 2,814.8 / 1,423.45 = 1.977 C over its own 21 C, short of
/// its 24 C, so flat out: it draws its 620 W, not the 2,814.8 W it gives.
/// The COP follows the sheet's table as the air it draws from cools: 2.63 at
/// -8.3 C (17 F), 3.585 at 0 C, its last point's 1.89 below -25 C. And the
/// bill is its draw: its recorded day is 620 W (`machines::day_energy`), and
/// the family home with it placed is 620 W a day-average worse off, not
/// 2,814.8, whether the time away charges that day or, with none recorded,
/// the meter's full draw (`day_power_balance`, both Ship life support modes).
///
/// Seen red (2026-10-20), each mutation alone: with the COP left out
/// (`heat::heaters` reading the heat pump as a resistance heater, its heat
/// its draw): "the room warmed 0.43556229264768775 K, by hand
/// 1.9774528086205028 K"; with the air step writing its heat as its draw
/// (`pc.draw_watts` from `heat_w`, heat charged as electricity): "it draws
/// its 620 W, not its heat: 2814.8"; and with the spawn giving it its heat
/// as its draw (`SpaceHeater::watts` = `heats_w`, so 4.54 x 2,814.8 W, which
/// holds the room at 24 C): "the room warmed 3 K, by hand 1.9774528086205028
/// K".
#[cfg(feature = "native")]
#[test]
fn a_heat_pump_delivers_its_cop_times_its_draw_and_is_charged_only_its_draw() {
    let data = one_room("room-a", 10.0, 3.0);
    let mut world = hecs::World::new();
    let pump = spawn_shipped(&mut world, "heat_pump", "heat_pump_t", [5.0, 0.5, 5.0]);
    let mut sys = FarmingSystem::new();
    run(&mut sys, &mut world, &data, 30, 5.0);
    let g = 6.24 * (2.0 * (10.0 + 10.0) * 3.0 + 10.0 * 10.0) + air_j_m3_k(21.0) * 0.5 * 300.0 / 3600.0;
    let want = 620.0 * 4.54 / g;
    let r = room(&world, "room-a");
    assert!((r.warmed_k - want).abs() < 1e-6, "the room warmed {} K, by hand {want} K", r.warmed_k);
    assert_eq!(r.heater, 1.0, "short of its 24 C, so flat out");
    assert!((draw(&world, pump) - 620.0).abs() < 1e-3, "it draws its 620 W, not its heat: {}", draw(&world, pump));
    // The Garden panel says both.
    world.spawn((crop(&data, "lettuce", "bed_a", 0),));
    let line = humidity::GuiView::new(&world, &data).areas.iter().find(|(a, _, _)| a == "bed_a").map(|l| l.1.clone()).unwrap();
    assert!(line.contains("heat pump flat out, 2,815 W of heat on 620 W, the air at 23.0 °C"), "{line}");

    // Its COP falls with the air it draws from, along the sheet's table.
    let d = air();
    let mut map = AirMap::new(&world, &data, &d);
    for (source_c, cop) in [(19.85, 4.54), (8.3, 4.54), (0.0, 2.63 + (4.54 - 2.63) * 8.3 / 16.6), (-8.3, 2.63), (-15.0, 2.2), (-30.0, 1.89)] {
        map.home_own_c = source_c;
        let h = crate::systems::farming::heat::heaters(&world, &map, &Default::default());
        assert_eq!(h.len(), 1);
        assert!((h[0].heat_w - 620.0 * cop).abs() < 1e-3, "drawing from {source_c} C: {} W of heat, by the sheet {} W", h[0].heat_w, 620.0 * cop);
        assert_eq!((h[0].watts, h[0].kind), (620.0, crate::systems::farming::heat::HeaterKind::HeatPump), "its draw stays 620 W");
    }

    // The bill is its draw, never its heat: a day of it recorded by the power tick.
    let mut rec = DataStore::new();
    crate::machines::day_energy::register(&mut rec);
    for _ in 0..24 {
        crate::machines::day_energy::record_world(&world, &rec, 3600.0);
    }
    let saved = crate::machines::day_energy::snapshot(&rec);
    let pump_day = saved.day_of("heat_pump_t", "heat_pump").map(|d| d.average_w());
    assert!(pump_day.is_some_and(|w| (w - 620.0).abs() < 1e-6), "its day is its draw: {pump_day:?}");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut home = crate::systems::grow_machines::load_home(&root.join("data/machines/home.ron")).expect("home.ron parses");
    let without = crate::systems::crafting::away::day_power_balance(&home, &Default::default());
    home.instances.push(crate::machines::MachineInstance {
        id: "heat_pump_t".into(),
        machine: "heat_pump".into(),
        room: String::new(),
        offset: (5.0, 0.5, 5.0),
        rotation: 0.0,
        zone: "home".into(),
        screen_source: None,
    });
    let meter = crate::systems::crafting::away::day_power_balance(&home, &Default::default());
    let away = crate::systems::crafting::away::day_power_balance(&home, &saved);
    for i in 0..2 {
        assert!((f64::from(without[i] - meter[i]) - 620.0).abs() < 0.05, "mode {i}: the meter charges it {} W", without[i] - meter[i]);
        assert!((f64::from(without[i] - away[i]) - 620.0).abs() < 0.05, "mode {i}: the time away charges it {} W", without[i] - away[i]);
    }
    assert_eq!(home.catalog["heat_pump"].average_load_watts(crate::machines::MeterBasis::default(), false), 620.0);
}

/// A WOOD STOVE WARMS THE AIR WHILE IT HAS WOOD AND STOPS WHEN ITS BOX IS EMPTY
/// (2026-10-20). home.ron's wood stove (the Drolet Escape 1800: 13,040 W of
/// heat on 4.07 kg of wood an hour), spawned with its box the way the engine
/// spawns it, one Wood Log (8 kg) stored in the box, in the family-sized
/// greenhouse. It has no power and needs none. By hand: flat out (it cannot
/// bring 11,209 W a degree to 24 C), the greenhouse heads for 13,040 / 11,209
/// = 1.163 C over, and is within 0.3% of it after an hour (its 5,607 m3 of air
/// takes 6.82 MJ a degree, a time constant of about ten minutes), when 4.07 kg
/// of the log has burned and the log is still in the box. The log is gone at
/// 8 / 4.07 = 1.966 hours; by 3 hours the box is empty, the stove has stopped,
/// the air has cooled back to within half a percent of its warmth (about six
/// time constants after the last log), and the Garden panel says why.
///
/// Seen red (2026-10-20) with `heat::heaters` reading no fuel in any box (a
/// stove never lit): "the greenhouse warmed 0 K, by hand 1.4193214327218364
/// K"; and with the step's burn never taken out of the box
/// (`heat::burn_stoves` not called, so it burned forever on one log): "an
/// hour burned 4.07 kg: 8 kg left".
#[cfg(feature = "native")]
#[test]
fn a_wood_stove_warms_the_air_while_it_has_wood_and_stops_when_its_box_is_empty() {
    let data = greenhouse_store();
    let mut world = hecs::World::new();
    let stove = spawn_shipped(&mut world, "wood_stove", "wood_stove_t", [20.0, 0.5, 10.0]);
    assert!(world.get::<&PowerConsumer>(stove).is_err(), "a wood stove draws no power");
    load_logs(&mut world, &data, stove, 1);
    let mut sys = FarmingSystem::new();
    run(&mut sys, &mut world, &data, 10, 5.0);
    // An hour of game time (10 ticks of 5 s at 72x) toward its steady 13,040 / G.
    let want = 13040.0 / greenhouse_w_k() * (1.0 - (-3600.0 * greenhouse_w_k() / (air_j_m3_k(21.0) * 5607.0)).exp());
    let r = room(&world, "room-greenhouse");
    assert!((r.warmed_k - want).abs() < 1e-3, "the greenhouse warmed {} K, by hand {want} K", r.warmed_k);
    assert_eq!(r.heater, 1.0, "flat out");
    // 4.07 as the data's f32 carries it.
    let rate = f64::from(4.07_f32);
    assert!((wood_kg(&world, stove) - (8.0 - rate)).abs() < 1e-9, "an hour burned 4.07 kg: {} kg left", wood_kg(&world, stove));
    assert_eq!(world.get::<&crate::systems::inventory::containers::Container>(stove).unwrap().current_qty, 1, "the log is still burning");
    world.spawn((crop(&data, "lettuce", "bed_a", 0),));
    let line = |world: &hecs::World| humidity::GuiView::new(world, &data).areas.iter().find(|(a, _, _)| a == "bed_a").map(|l| l.1.clone()).unwrap();
    let burning = line(&world);
    assert!(burning.contains("stove flat out, 13,040 W of heat from 4.1 kg of wood an hour"), "{burning}");

    run(&mut sys, &mut world, &data, 20, 5.0);
    {
        let c = world.get::<&crate::systems::inventory::containers::Container>(stove).unwrap();
        assert!(c.current_qty == 0 && c.current_content_item.is_none(), "after 3 hours the box still holds {:?} x {}", c.current_content_item, c.current_qty);
    }
    assert_eq!(wood_kg(&world, stove), 0.0);
    let r = room(&world, "room-greenhouse");
    assert!(r.warmed_k < 0.005 * 13040.0 / greenhouse_w_k(), "with no wood the greenhouse cools back: {} K over", r.warmed_k);
    assert_eq!(r.heater, 0.0, "the stove is out");
    let out = line(&world);
    assert!(out.contains("stove off (no wood), the air at 21.0 °C"), "{out}");
}

/// A WOOD STOVE BURNS THE SAME WOOD AT 30 AND AT 240 FRAMES A SECOND
/// (2026-10-20), the way a fire does (src/systems/fire.rs, BUG-164's note):
/// the wood stove with two Wood Logs (16 kg) in the greenhouse, run three
/// game hours at 30 frames a second and at 240, burns the same wood to a
/// billionth of a kilogram, 3 x 4.07 = 12.21 kg by hand, and warms the air
/// the same; run on to five hours, both boxes are empty.
///
/// Seen red (2026-10-20) with the box's partial log kept in f32
/// (`FuelStove::burned_kg` added to as f32 in `heat::burn_stoves`, the way the
/// generators' drum accumulator works): "30 fps left 3.7900547981262207 kg,
/// 240 fps 3.7905921936035156 kg"; and with no fuel read from any box: "three
/// hours burned 12.21 kg: 16 kg left".
#[cfg(feature = "native")]
#[test]
fn a_wood_stove_burns_the_same_wood_at_30_and_240_frames_a_second() {
    let mut ends = Vec::new();
    for fps in [30.0_f32, 240.0] {
        let data = greenhouse_store();
        let mut world = hecs::World::new();
        let stove = spawn_shipped(&mut world, "wood_stove", "wood_stove_t", [20.0, 0.5, 10.0]);
        load_logs(&mut world, &data, stove, 2);
        let mut sys = FarmingSystem::new();
        // 72 game seconds a real second: three game hours are 150 real seconds.
        let ticks = (150.0 * fps).round() as usize;
        run(&mut sys, &mut world, &data, ticks, 1.0 / fps);
        let at3 = (wood_kg(&world, stove), room(&world, "room-greenhouse").warmed_k);
        run(&mut sys, &mut world, &data, ticks * 2 / 3, 1.0 / fps);
        let c = world.get::<&crate::systems::inventory::containers::Container>(stove).unwrap();
        ends.push((at3, c.current_qty, wood_kg(&world, stove)));
    }
    let ((wood30, warm30), qty30, end30) = ends[0];
    let ((wood240, warm240), qty240, end240) = ends[1];
    assert!((wood30 - wood240).abs() < 1e-9, "30 fps left {wood30} kg, 240 fps {wood240} kg");
    assert!((wood30 - (16.0 - 3.0 * 4.07)).abs() < 1e-5, "three hours burned 12.21 kg: {wood30} kg left");
    assert!((warm30 - warm240).abs() < 1e-6, "the air warmed {warm30} K at 30 fps, {warm240} K at 240");
    assert_eq!((qty30, qty240, end30, end240), (0, 0, 0.0, 0.0), "five hours burn both boxes empty");
}

/// Two sealed homes, one with no stove and one with a stove fitted for one
/// kind of combustion air, after the same hour (`burn_a_log_in_a_sealed_home`).
#[cfg(feature = "native")]
struct SealedHomes {
    plain: crate::ecs::components::HomeAirState,
    stoved: crate::ecs::components::HomeAirState,
    /// The wood the stove burned, kg.
    burned: f64,
    /// The Garden panel's home lines, joined by " / ".
    plain_panel: String,
    panel: String,
    /// The oxygen the ship's supply ledger metered against the stove's home, kg.
    ship_o2_kg: f64,
}

/// Two sealed 1,000 m3 homes with nobody breathing in them, the life-support
/// data and the ship's supply ledger, one with home.ron's wood stove fitted
/// for `air` and one Wood Log in its own air, each run an hour (ten 5 s
/// ticks, six game minutes each). The stove burns flat out: the home loses
/// 3,447 W a degree, so 13,040 W never brings it to 24 C.
#[cfg(feature = "native")]
fn burn_a_log_in_a_sealed_home(air: crate::ecs::components::CombustionAir) -> SealedHomes {
    use crate::systems::life_support::{LifeSupportData, DATA_KEY, LIFE_SUPPORT_RON};
    use crate::systems::ship_power::{ShipSupplyLedger, LEDGER_KEY, OXYGEN, PLAYER_HOME};
    let mut def = shipped("home.ron", "wood_stove");
    def.burns_fuel.as_mut().expect("the wood stove burns fuel").combustion_air = air;
    let mut homes = Vec::new();
    for with_stove in [false, true] {
        let mut data = store(Vec::new(), Vec::new());
        data.insert(DATA_KEY, LifeSupportData::parse(LIFE_SUPPORT_RON).expect("life_support.ron parses"));
        data.insert(LEDGER_KEY, Mutex::new(ShipSupplyLedger::default()));
        let mut world = hecs::World::new();
        world.spawn((HomeAir { metabolic_kcal_per_day: 0.0, ..Default::default() }, EnclosedSpace::new_sealed(1000.0)));
        let stove = with_stove.then(|| {
            let s = spawn_def(&mut world, &def, "wood_stove", "wood_stove_t", [50.0, 0.5, 50.0]);
            load_logs(&mut world, &data, s, 1);
            s
        });
        let mut sys = FarmingSystem::new();
        run(&mut sys, &mut world, &data, 10, 5.0);
        let burned = stove.map_or(0.0, |s| 8.0 - wood_kg(&world, s));
        let panel = humidity::GuiView::new(&world, &data).home.into_iter().map(|(l, _)| l).collect::<Vec<_>>().join(" / ");
        let ship = crate::systems::ship_power::tally(&data, PLAYER_HOME, OXYGEN).drawn;
        homes.push((memory(&world).home_air, burned, panel, ship));
    }
    let (stoved, burned, panel, ship_o2_kg) = homes.pop().unwrap();
    let (plain, _, plain_panel, plain_ship) = homes.pop().unwrap();
    assert_eq!(plain_ship, 0.0, "no stove, nothing metered");
    SealedHomes { plain, stoved, burned, plain_panel, panel, ship_o2_kg }
}

/// The data's figures, as its f32 carries them: the stove's kilograms of wood
/// an hour, and the oxygen each kilogram burns.
fn stove_rates() -> (f64, f64) {
    (f64::from(4.07_f32), f64::from(1.094_f32))
}

/// THE SHIPPED WOOD STOVE TAKES NONE OF THE HOME'S OXYGEN: ITS FIRE'S AIR COMES
/// FROM OUTSIDE THROUGH ITS KIT (2026-10-20). home.ron's wood stove as shipped
/// (`combustion_air: Outside`), flat out for an hour in a sealed 1,000 m3
/// home, burns 4.07 kg of wood and 4.07 x 1.094 = 4.453 kg of oxygen, all of
/// it from the ship's air beyond the home: the stove's home has the same
/// oxygen as the home with no stove but for the leak's share of a warmer air
/// (a few millionths of a gram a cubic metre, where its burn is 4.45 g), the
/// same oxygen used a day, and none of it counted as the stove's in the home's
/// air; the ship's supply ledger meters the 4.453 kg against the home, and the
/// home keeps the stove's 106.9 kg a day apart as the ship's. Its carbon
/// dioxide and water go up the flue, as before, and it still warms the home.
///
/// Seen red (2026-10-20) with the code before the fix in effect (`heat::burn`
/// sending every stove's oxygen to the home's air whatever its data, and the
/// panel's words for a stove's air left out): "the stove's home lost
/// 4.452580815071769 g/m3 more oxygen than the home with no stove; its fire's
/// air comes from outside".
#[cfg(feature = "native")]
#[test]
fn the_shipped_stove_takes_none_of_the_homes_oxygen_its_air_comes_from_outside() {
    use crate::ecs::components::CombustionAir;
    let def = shipped("home.ron", "wood_stove");
    assert_eq!(def.burns_fuel.as_ref().map(|b| b.combustion_air), Some(CombustionAir::Outside), "home.ron's stove has its kit");
    let solo = shipped("home_solo.ron", "wood_stove");
    assert_eq!(solo.burns_fuel.as_ref().map(|b| b.combustion_air), Some(CombustionAir::Outside), "home_solo.ron's stove has its kit");

    let h = burn_a_log_in_a_sealed_home(CombustionAir::Outside);
    let (rate, o2_per_kg) = stove_rates();
    assert!((h.burned - rate).abs() < 1e-9, "flat out, an hour burned {} kg", h.burned);
    let lost = h.plain.o2_g_m3 - h.stoved.o2_g_m3;
    assert!(lost.abs() < 1e-4, "the stove's home lost {lost} g/m3 more oxygen than the home with no stove; its fire's air comes from outside");
    assert!(
        (h.stoved.o2_used_kg_day - h.plain.o2_used_kg_day).abs() < 1e-9,
        "the home's oxygen used a day is not the fire's: {} kg a day more",
        h.stoved.o2_used_kg_day - h.plain.o2_used_kg_day
    );
    assert_eq!(h.stoved.stove_o2_kg_day, 0.0, "none of the home's air burned by the stove");
    assert!(
        (h.stoved.stove_ship_o2_kg_day - rate * o2_per_kg * 24.0).abs() < 1e-6,
        "the ship's air feeds it {} kg a day, flat out 106.9",
        h.stoved.stove_ship_o2_kg_day
    );
    assert!((h.ship_o2_kg - h.burned * o2_per_kg).abs() < 1e-9, "the ship's ledger meters {} kg of oxygen, by its burn {}", h.ship_o2_kg, h.burned * o2_per_kg);
    assert!((h.stoved.co2_g_m3 - h.plain.co2_g_m3).abs() < 1e-6, "no carbon dioxide in the home's air: {} against {}", h.stoved.co2_g_m3, h.plain.co2_g_m3);
    assert!((h.stoved.vapour_g_m3 - h.plain.vapour_g_m3).abs() < 1e-6, "nor the fire's water: {} against {}", h.stoved.vapour_g_m3, h.plain.vapour_g_m3);
    assert!(h.stoved.warmed_k > 3.0, "and it warmed the home: {} K", h.stoved.warmed_k);
}

/// A STOVE WITH NO OUTSIDE AIR KIT STILL BURNS THE HOME'S OXYGEN, AT THE RATE IT
/// ALWAYS DID (2026-10-20): the danger the heating guide teaches stays real in
/// the game. The same stove with `combustion_air: Room`, flat out for an hour
/// in the same sealed home: the stove's home has lost, beyond the other,
/// exactly 1.094 kg of oxygen for each kilogram it burned (the oxygen dry
/// wood's carbon and hydrogen take, at 20% moisture: data/machines/home.ron),
/// 4.453 g a cubic metre, and counts it in the day's oxygen used (flat out,
/// 4.07 x 1.094 x 24 = 106.9 kg a day), kept apart as the stove's; the ship
/// meters nothing; its carbon dioxide and water are not in the home's air,
/// which differs from the other's only by the leak's share of a warmer air.
/// These are the figures v0.1528.0's test held for the shipped stove before
/// it had its kit, and with the code before the fix in effect this test
/// passed: a room-air stove behaves exactly as every stove did then.
///
/// Seen red (2026-10-20) with the fix applied to every stove whatever its
/// data (`heat::burn` sending every stove's oxygen to the ship's air): "the
/// stove's home lost 0.000002104627526477998 g/m3 more oxygen, by its burn
/// 4.452580114049909 g/m3" (the 2.1 millionths is the leak's share of the
/// warmer air). The v0.1528.0 test of the same figures was seen red with the
/// step's oxygen never taken from the home's air, with the fire's carbon
/// dioxide put into the home's air ("no carbon dioxide in the home's air:
/// 6.698622774424283 against 0.7320045079894062"), and with the stove's share
/// not kept apart.
#[cfg(feature = "native")]
#[test]
fn a_stove_with_no_kit_still_burns_the_homes_oxygen_at_the_rate_it_always_did() {
    let h = burn_a_log_in_a_sealed_home(crate::ecs::components::CombustionAir::Room);
    let (rate, o2_per_kg) = stove_rates();
    assert!((h.burned - rate).abs() < 1e-9, "flat out, an hour burned {} kg", h.burned);
    let lost = h.plain.o2_g_m3 - h.stoved.o2_g_m3;
    let by_burn = h.burned * o2_per_kg * 1000.0 / 1000.0;
    assert!((lost - by_burn).abs() < 1e-3, "the stove's home lost {lost} g/m3 more oxygen, by its burn {by_burn} g/m3");
    assert!(
        (h.stoved.o2_used_kg_day - h.plain.o2_used_kg_day - rate * o2_per_kg * 24.0).abs() < 1e-6,
        "the day's oxygen used counts the fire: {} kg a day more",
        h.stoved.o2_used_kg_day - h.plain.o2_used_kg_day
    );
    assert!((h.stoved.stove_o2_kg_day - rate * o2_per_kg * 24.0).abs() < 1e-6, "kept apart as the stove's: {}", h.stoved.stove_o2_kg_day);
    assert_eq!((h.stoved.stove_ship_o2_kg_day, h.ship_o2_kg), (0.0, 0.0), "the ship's air feeds it nothing, and nothing is metered");
    assert!((h.stoved.co2_g_m3 - h.plain.co2_g_m3).abs() < 1e-6, "no carbon dioxide in the home's air: {} against {}", h.stoved.co2_g_m3, h.plain.co2_g_m3);
    assert!((h.stoved.vapour_g_m3 - h.plain.vapour_g_m3).abs() < 1e-6, "nor the fire's water: {} against {}", h.stoved.vapour_g_m3, h.plain.vapour_g_m3);
    assert!(h.stoved.warmed_k > 3.0, "and it warmed the home: {} K", h.stoved.warmed_k);
}

/// THE GARDEN PANEL SAYS WHERE A STOVE'S AIR COMES FROM (2026-10-20). The two
/// stoves above, through the Garden panel's home lines: the shipped one's
/// heater line says its air comes from outside through its kit, and its day
/// line keeps the fire out of the home's balance and says what the ship's air
/// gave it; the one with no kit says its air comes from this room, and its
/// day line says what the stove burned beside what is breathed in. With no
/// stove, neither line mentions one.
///
/// Seen red (2026-10-20) with the code before the fix in effect (no words for
/// a stove's air, and every stove's oxygen taken from the home's air): the
/// shipped stove's lines read "... the crops give out 0.0 kg of oxygen and
/// 0.0 kg is breathed in and 106.9 kg burned by the stove, so the home's
/// oxygen loses 106.86 kg a day / ... / Home air: stove flat out, 13,040 W of
/// heat from 4.1 kg of wood an hour, the air at 23.6 °C, short of the 24 °C it
/// is set to", with no word of where its air comes from. And red with the fix
/// applied to every stove whatever its data (the words right, the oxygen sent
/// to the ship's air for both): the stove with no kit read "Home air: stove
/// flat out, 13,040 W of heat from 4.1 kg of wood an hour and its air from
/// this room" while its day line said "0.0 kg is breathed in, so the home's
/// oxygen gains 0.00 kg a day; the stove burns 106.9 kg more from the ship's
/// air, through its outside air kit".
#[cfg(feature = "native")]
#[test]
fn the_garden_panel_says_where_a_stoves_air_comes_from() {
    use crate::ecs::components::CombustionAir;
    let outside = burn_a_log_in_a_sealed_home(CombustionAir::Outside);
    assert!(
        outside.panel.contains("Home air: stove flat out, 13,040 W of heat from 4.1 kg of wood an hour and its air from outside through its kit, the air at"),
        "{}",
        outside.panel
    );
    assert!(
        outside.panel.contains("0.0 kg is breathed in, so the home's oxygen gains 0.00 kg a day; the stove burns 106.9 kg more from the ship's air, through its outside air kit"),
        "{}",
        outside.panel
    );
    assert!(!outside.panel.contains("burned by the stove"), "{}", outside.panel);
    assert!(!outside.plain_panel.contains("stove"), "{}", outside.plain_panel);

    let room = burn_a_log_in_a_sealed_home(CombustionAir::Room);
    assert!(
        room.panel.contains("Home air: stove flat out, 13,040 W of heat from 4.1 kg of wood an hour and its air from this room, the air at"),
        "{}",
        room.panel
    );
    // The day line says the fire burned it, not that it was breathed.
    assert!(room.panel.contains("0.0 kg is breathed in and 106.9 kg burned by the stove"), "{}", room.panel);
    assert!(!room.panel.contains("ship's air"), "{}", room.panel);
}

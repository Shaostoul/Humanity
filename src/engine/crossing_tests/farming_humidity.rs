//! From systems/farming/humidity_tests.rs: a room resized on the meter, its fans spawned the way the
//! engine spawns them (`engine::home_spawn`).

use std::collections::HashMap;
use std::sync::Mutex;
use humanity_core::ecs::components::{Irrigator, PowerConsumer};
use humanity_core::ecs::systems::System;
use humanity_core::hot_reload::data_store::DataStore;
use humanity_systems::farming::humidity::{self, GrowRoom};
use humanity_systems::farming::fixtures::humidity::*;
use humanity_systems::farming::*;

/// A ROOM RESIZED CHANGES ITS FAN'S DAY ON THE METER, WITH NO DATA EDIT
/// (2026-10-22, the operator's "Do rooms not automatically calculate their
/// volumes on the fly?"). home.ron's exhaust fan (250 W, 2,725 m3 an hour;
/// its data's day is 0 W, idle in the shipped greenhouse, whose air handlers
/// hold it under the fan's setpoint) is spawned the way the engine spawns it
/// in the 10 x 3 x 10 m Greenhouse over 140 units of tomatoes (322 L a day
/// breathed out, about 1,700 m3 an hour of the home's air to carry it off at
/// the fan's 80%, so the fan runs at about half speed), with the machines'
/// power tick recording what it draws (`machines::day_energy`). After a game
/// day the power meter charges it what it drew, measured: the day's mean of
/// the draw its controller set, not its data's 0 W. Then the room is
/// published twice as long, 20 x 10 m, as a player's edit republishes it, and
/// a day later the meter charges the new day: the bigger floor's walls trade
/// twice the air with the home (half an air change of the room an hour),
/// which carries off more of the crops' water, so the fan runs slower, and
/// its draw goes as the cube of its speed. The data never changes.
///
/// Measured (2026-10-23): the fan's day is 47.7 W in the 10 x 10 m room and
/// 35.3 W in the 20 x 10 m one.
///
/// Seen red (2026-10-23) with `MachineHome::day_draw` reading only the data:
/// "after a day in the 10 x 10 m room the meter charges the fan 0 W
/// (Estimated { hours: 24.0 }), the day it drew averaged 47.7 W".
#[cfg(feature = "native")]
#[test]
fn a_room_resized_changes_its_fans_day_on_the_meter() {
    use humanity_core::ecs::components::{PowerCircuit, PowerGenerator};
    use humanity_ship::machines::{DaySource, MachineInstance, MachinePower, MeterBasis};
    let mut data = store(0.5, 1.0, 0.0);
    data.insert(units::PLOT_AREA_KEY, HashMap::from([("bed_a".to_string(), 1.0_f32)]));
    humanity_ship::machines::day_energy::register(&mut data);
    data.insert("power_status", Mutex::new(humanity_systems::electrical::PowerStatus::default()));
    let root = std::path::Path::new(env!("HUMANITY_REPO_ROOT"));
    let mut home = humanity_systems::grow_machines::load_home(&root.join("data/machines/home.ron")).expect("home.ron parses");
    let fan_def = home.catalog["exhaust_fan"].clone();
    let Some(MachinePower::Consumer { average_watts: Some(estimate), .. }) = fan_def.power else { panic!("the shipped fan carries its data's day") };
    let inst = MachineInstance { id: "fan_t".into(), machine: "exhaust_fan".into(), room: "room-a".into(), offset: (5.0, 2.2, 9.5), rotation: 0.0, zone: "home".into(), screen_source: None };
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    for slot in 0..140 {
        world.spawn((crop(&data, "tomato", "bed_a", slot),));
    }
    let empty = HashMap::new();
    crate::engine::home_spawn::spawn_home_machine_entity(&mut world, &inst, &fan_def, &empty, &empty, None, None);
    home.instances.push(inst);
    world.spawn((PowerGenerator { output_watts: 10_000.0, fuel_per_second: 0.0, active: true }, PowerCircuit { island: 0 }));
    let fan = world.query::<(&humanity_core::ecs::components::MachineInstanceId, &PowerConsumer)>().iter().find(|(_, (id, _))| id.0 == "fan_t").map(|(e, _)| e).unwrap();
    let mut power = humanity_systems::electrical::ElectricalSystem::new(std::path::Path::new("data"));
    let mut sys = FarmingSystem::new();
    // Two game days, a tick 2 s at the store's 72x (144 game s, 25 to the hour), and the mean
    // of the draw each power tick recorded over the second, which is the record's day.
    let mut two_days = |world: &mut hecs::World| -> f64 {
        let mut sum = 0.0;
        for tick in 0..1200 {
            let drawn = world.get::<&PowerConsumer>(fan).map(|pc| if pc.enabled { f64::from(pc.draw_watts) } else { 0.0 }).unwrap();
            if tick >= 600 {
                sum += drawn;
            }
            power.tick(world, 2.0, &data);
            sys.tick(world, 2.0, &data);
        }
        sum / 600.0
    };
    let fan_on_meter = |data: &DataStore| {
        let recorded = humanity_ship::machines::day_energy::snapshot(data);
        let meter = home.utility_meters(4.5, MeterBasis { life_support_on_grid: false, recorded: Some(&recorded) });
        let power = meter.into_iter().find(|m| m.utility == "power").expect("a power meter");
        power.sources.into_iter().find(|s| s.id == "fan_t").expect("the fan is on the meter").draw
    };

    let small_mean = two_days(&mut world);
    let small = fan_on_meter(&data);
    assert!(
        small.source == DaySource::Measured && (f64::from(small.watts) - small_mean).abs() < 1e-3 * small_mean.max(1.0),
        "after a day in the 10 x 10 m room the meter charges the fan {} W ({:?}), the day it drew averaged {small_mean:.1} W",
        small.watts,
        small.source
    );
    assert!(small_mean > 10.0 && small_mean < 240.0, "the fan works at part speed in this room: {small_mean} W");

    // The room as a player's edit republishes it, twice as long.
    *data.get::<Mutex<Vec<GrowRoom>>>(humidity::ROOMS_KEY).unwrap().lock().unwrap() =
        vec![GrowRoom { id: "room-a".into(), name: "Greenhouse".into(), min: [0.0, 0.0, 0.0], max: [20.0, 3.0, 10.0], ..Default::default() }];
    let big_mean = two_days(&mut world);
    let big = fan_on_meter(&data);
    assert!(
        big.source == DaySource::Measured && (f64::from(big.watts) - big_mean).abs() < 1e-3 * big_mean.max(1.0),
        "a day in the 20 x 10 m room: the meter charges {} W, the day it drew averaged {big_mean:.1} W",
        big.watts
    );
    println!("the fan's day: {} W in the 10 x 10 m room, {} W in the 20 x 10 m one", small.watts, big.watts);
    assert!(
        big.watts < 0.8 * small.watts,
        "the meter follows the room: the fan's day was {} W in the 10 x 10 m room and is {} W in the 20 x 10 m one",
        small.watts,
        big.watts
    );
    let Some(MachinePower::Consumer { average_watts: Some(after), .. }) = home.catalog["exhaust_fan"].power else { panic!() };
    assert_eq!(after, estimate, "the data never changed");
}

//! The day record (systems::day_energy) and the meters that read it. The
//! simulation-driven tests (a room resized changes its fan's day; the time
//! away charges the recorded day) are beside the systems they run:
//! `farming::humidity_tests` and `farming::heat_tests`.

use super::*;
use crate::machines::{DaySource, MachineDef, MachineHome, MachineInstance, MachinePower, MeterBasis};

/// A day's need is only what it drew and asked for over the LAST 24 whole
/// hours: an hour in progress is not counted, the oldest hour leaves as a new
/// one completes, a stretch is split across the hours it spans, and the hours
/// a machine sat shed are kept apart. Seen red (2026-10-23) with `day`
/// summing the hour in progress: "the hour in progress is not counted:
/// DayFigure { drawn_wh: 2450.0, shed_wh: 0.0 }".
#[test]
fn the_record_is_the_last_twenty_four_whole_hours() {
    let mut d = MachineDay::default();
    // 23.5 hours at 100 W: not a day yet.
    d.add(100.0, 100.0, 23.5 * HOUR_S);
    assert_eq!(d.day(), None, "23.5 hours is not a day");
    assert!((d.hours_recorded() - 23.5).abs() < 1e-9);
    // Another hour: the 24th completes half way through it.
    d.add(100.0, 100.0, HOUR_S);
    let day = d.day().expect("24 whole hours");
    assert!((day.need_wh() - 2400.0).abs() < 1e-6, "the hour in progress is not counted: {day:?}");
    // The hour in progress finished, then a day at 50 W, the last 6 hours of
    // it shed (asked for, not drawn): the 100 W hours leave, one by one.
    d.add(100.0, 100.0, 0.5 * HOUR_S);
    d.add(50.0, 50.0, 18.0 * HOUR_S);
    d.add(0.0, 50.0, 6.0 * HOUR_S);
    let day = d.day().unwrap();
    assert!((day.need_wh() - 1200.0).abs() < 1e-6, "the day at 50 W: {day:?}");
    assert!((day.drawn_wh - 900.0).abs() < 1e-6 && (day.shed_wh - 300.0).abs() < 1e-6, "6 hours of 50 W shed: {day:?}");
    assert!((day.average_w() - 50.0).abs() < 1e-9);
    assert_eq!(d.hours.len(), DAY_HOURS);
}

/// A day of ticks makes a whole day, however the time is sliced, and a
/// stretch longer than a day leaves a day of it. A tenth of a second is not a
/// whole number in binary, so a day of 0.1 s ticks does not add up to the hour
/// exactly. Seen red (2026-10-23) with the hour completing only on the exact
/// second (`take >= room`): "23 whole hours from a day of 0.1 s ticks". (A
/// day of 1/60 s frames happens to land exactly, which is why the ticks here
/// are tenths.)
#[test]
fn a_day_of_ticks_makes_a_whole_day() {
    let mut a = MachineDay::default();
    let mut b = MachineDay::default();
    for _ in 0..(24 * 3600 * 10) {
        a.add(37.0, 37.0, 0.1);
    }
    b.add(37.0, 37.0, 24.0 * HOUR_S);
    assert_eq!(a.hours.len(), DAY_HOURS, "{} whole hours from a day of 0.1 s ticks", a.hours.len());
    assert!((a.day().unwrap().need_wh() - b.day().unwrap().need_wh()).abs() < 1e-6);
    let mut long = MachineDay::default();
    long.add(10.0, 10.0, 1000.0 * HOUR_S);
    assert!((long.day().unwrap().need_wh() - 240.0).abs() < 1e-9);
}

/// The record is kept by instance id and type: a different machine placed
/// under the same id starts a new day, and a whole hour moves the revision on
/// (so the pages copy it), a restore too.
#[test]
fn a_new_machine_under_an_old_id_starts_afresh() {
    let mut r = DayEnergy::default();
    r.record("fan_1", "exhaust_fan", 40.0, 40.0, 24.0 * HOUR_S);
    assert!(r.day_of("fan_1", "exhaust_fan").is_some());
    assert_eq!(r.day_of("fan_1", "heater"), None, "the day is the fan's, not a heater's");
    let rev = r.revision;
    assert!(rev > 0, "whole hours move the revision on");
    r.record("fan_1", "heater", 1500.0, 1500.0, HOUR_S / 2.0);
    assert_eq!(r.day_of("fan_1", "exhaust_fan"), None, "a heater placed under the id starts afresh");
    assert!((r.hours_of("fan_1", "heater") - 0.5).abs() < 1e-9);
    assert_eq!(r.revision, rev, "half an hour does not move it");

    let mut data = DataStore::new();
    register(&mut data);
    restore(&data, &r);
    assert_eq!(snapshot(&data).machines, r.machines);
    assert!(revision(&data) > 0, "a restore moves it on");
}

/// The machines' power tick records each placed machine's draw while its
/// island powers it and its request while shed, on the game clock it ran.
/// Seen red (2026-10-23) with the ElectricalSystem not calling `record_world`
/// (the record stayed empty): "the fan's day: None".
#[test]
fn the_power_tick_records_what_each_machine_drew() {
    use crate::ecs::components::{MachineInstanceId, MachineType, PowerConsumer, PowerGenerator};
    use crate::ecs::systems::System;
    let mut data = DataStore::new();
    register(&mut data);
    data.insert("power_status", std::sync::Mutex::new(crate::systems::electrical::PowerStatus::default()));
    let mut world = hecs::World::new();
    world.spawn((PowerGenerator { output_watts: 100.0, fuel_per_second: 0.0, active: true },));
    world.spawn((MachineInstanceId("fan".into()), MachineType("exhaust_fan".into()), PowerConsumer { draw_watts: 60.0, priority: 1, enabled: true }));
    world.spawn((MachineInstanceId("lamp".into()), MachineType("grow_light".into()), PowerConsumer { draw_watts: 80.0, priority: 5, enabled: true }));
    let mut power = crate::systems::electrical::ElectricalSystem::new(std::path::Path::new("data"));
    // No clock in the store, so a tick's seconds are game seconds: a day of minutes.
    for _ in 0..(24 * 60) {
        power.tick(&mut world, 60.0, &data);
    }
    let r = snapshot(&data);
    let fan = r.day_of("fan", "exhaust_fan").unwrap_or_else(|| panic!("the fan's day: {:?}", r.machines.get("fan").map(|d| d.hours.len())));
    assert!((fan.drawn_wh - 60.0 * 24.0).abs() < 1e-6, "the fan, fed first, drew its 60 W all day: {fan:?}");
    // The island's 100 W cannot carry both: the optional lamp is shed and asks all day.
    let lamp = r.day_of("lamp", "grow_light").unwrap();
    assert!(lamp.drawn_wh.abs() < 1e-6 && (lamp.shed_wh - 80.0 * 24.0).abs() < 1e-6, "the lamp, shed: {lamp:?}");
    assert!((lamp.average_w() - 80.0).abs() < 1e-9, "its need is what it asked for");
}

fn consumer(watts: f32, average_watts: Option<f32>) -> MachineDef {
    let mut d: MachineDef = ron::from_str(r#"(shape: "box", size: (1.0, 1.0, 1.0), color: (0.5, 0.5, 0.5))"#).expect("a bare def");
    d.power = Some(MachinePower::Consumer { watts, priority: 2, idle_watts: None, average_watts });
    d
}

fn placed(id: &str, machine: &str) -> MachineInstance {
    MachineInstance { id: id.into(), machine: machine.into(), room: String::new(), offset: (0.0, 0.0, 0.0), rotation: 0.0, zone: "home".into(), screen_source: None }
}

/// THE METER CHARGES A RECORDED DAY AND SAYS WHICH FIGURES ARE MEASURED. Two
/// fans whose data says 20 W a day: one has run a whole day at 50 W, so the
/// meter charges it 1.2 kWh, measured; the other has run 5 hours, so it is
/// still its data's 0.48 kWh, estimated, and the card says how far along it
/// is. Seen red (2026-10-23) with `day_draw` reading only the data (the meter
/// charged both 0.48 kWh): "1.2 kWh measured and 0.48 estimated, not 0.96".
#[test]
fn the_meter_charges_a_recorded_day_and_says_which_figures_are_measured() {
    let mut home: MachineHome = ron::from_str("(catalog: {}, instances: [], connections: [])").expect("an empty home");
    let mut fan = consumer(70.0, Some(20.0));
    fan.label = "Exhaust fan".into();
    fan.ventilation_m3_h = 1000.0;
    home.catalog.insert("exhaust_fan".into(), fan);
    home.instances.push(placed("fan_a", "exhaust_fan"));
    home.instances.push(placed("fan_b", "exhaust_fan"));
    let mut r = DayEnergy::default();
    r.record("fan_a", "exhaust_fan", 50.0, 50.0, 25.0 * HOUR_S);
    r.record("fan_b", "exhaust_fan", 50.0, 50.0, 5.0 * HOUR_S);
    let basis = MeterBasis { life_support_on_grid: false, recorded: Some(&r) };
    let power = home.utility_meters(4.5, basis).into_iter().find(|m| m.utility == "power").expect("a power meter");
    assert!((power.demand - (1.2 + 0.48)).abs() < 1e-4, "1.2 kWh measured and 0.48 estimated, not {}", power.demand);
    assert_eq!(power.measured_and_estimated(), (1, 1));
    assert!(power.summary.contains("1 machine's draw measured over the last game day"), "{}", power.summary);
    let [measured, estimated] = power.source_groups(&home.catalog);
    assert_eq!(measured.len(), 1);
    assert_eq!((measured[0].0.as_str(), measured[0].1), ("Exhaust fan", 1));
    assert_eq!(estimated[0].1, 1);
    let b = home.day_draw(&home.instances[1], &home.catalog["exhaust_fan"], Some(&r));
    assert_eq!(b.source, DaySource::Estimated { hours: 5.0 });
    assert_eq!(crate::machines::day_draw_line(&b), "about 0.48 kWh a day, its data's estimate; measured once it has run a whole game day (5 of 24 hours so far)");
    let a = home.day_draw(&home.instances[0], &home.catalog["exhaust_fan"], Some(&r));
    assert_eq!(crate::machines::day_draw_line(&a), "1.20 kWh a day, measured over its last game day");
    // The Air loop's line on the Home page reads the same figures.
    let air = home.loop_meter_line(crate::machines::LoopMeter::Air, 4.5, basis).expect("an air line");
    assert_eq!(air, "its machines draw 1.7 kWh/day (Exhaust fan x2, 1.7 kWh/day); 1 machine's draw measured over the last game day, 1 machine's estimated from their data until they have run one");
    // Nothing recorded: every figure from the data, as before.
    let none = home.utility_meters(4.5, MeterBasis::default()).into_iter().find(|m| m.utility == "power").unwrap();
    assert!((none.demand - 0.96).abs() < 1e-4, "{}", none.demand);
}

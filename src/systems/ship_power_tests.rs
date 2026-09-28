//! The ship's reactor feed (2026-09-27). Each test says how it was seen red.

use super::*;
use crate::ecs::components::{Battery, PowerCircuit, PowerConsumer, PowerGenerator};
use crate::ecs::systems::System;
use crate::systems::electrical::{ElectricalSystem, PowerStatus};
use std::sync::Mutex;

/// A DataStore with the reactor registered and the Ship life support mode
/// set: `realistic` false is the default, Station-supplied.
fn store(realistic: bool) -> DataStore {
    let mut data = DataStore::new();
    data.insert("power_status", Mutex::new(PowerStatus::default()));
    crate::systems::life_support::register(&mut data);
    crate::systems::life_support::publish(&data, realistic);
    data
}

fn drawn(data: &DataStore) -> SupplyTally {
    tally(data, PLAYER_HOME, POWER)
}

fn status(data: &DataStore) -> PowerStatus {
    *data.get::<Mutex<PowerStatus>>("power_status").unwrap().lock().unwrap()
}

/// The shipped reactor parses and is the figure its data file quotes.
#[test]
fn the_shipped_reactor_is_one_klt_40s() {
    let d = ShipPowerData::parse(SHIP_POWER_RON).expect("parses");
    assert_eq!(d.electric_watts, 35_000_000.0);
    assert!(d.reactor.contains("KLT-40S"));
}

/// A HOME ON THE SHIP'S REACTOR NEVER SHEDS, AND EVERY WATT-HOUR IS METERED.
/// Station-supplied: an island with 150 W of its own and 2,500 W of loads
/// (an optional 500 W and a critical 2,000 W) keeps both on for a game hour
/// of one-second ticks, and the ledger holds exactly the 2,350 Wh the reactor
/// gave, to the watt-hour; the Live power card reads 2,350 W now. Seen red
/// with the feed left out of the island's supply (the 500 W load was shed
/// and the 2,000 W one too), and with the metering on raw `f32` (the total
/// drifted).
#[test]
fn a_fed_home_never_sheds_and_is_metered_to_the_watt_hour() {
    let data = store(false);
    let mut world = hecs::World::new();
    spawn_feed_taps(&mut world, PLAYER_HOME, [0]);
    world.spawn((PowerGenerator { output_watts: 150.0, fuel_per_second: 0.0, active: true }, PowerCircuit { island: 0 }));
    let optional = world.spawn((PowerConsumer { draw_watts: 500.0, priority: 5, enabled: true }, PowerCircuit { island: 0 }));
    let critical = world.spawn((PowerConsumer { draw_watts: 2000.0, priority: 1, enabled: true }, PowerCircuit { island: 0 }));
    let mut sys = ElectricalSystem::new(std::path::Path::new("data"));
    for _ in 0..3600 {
        sys.tick(&mut world, 1.0, &data);
    }
    assert!(world.get::<&PowerConsumer>(optional).unwrap().enabled, "the optional load keeps its power");
    assert!(world.get::<&PowerConsumer>(critical).unwrap().enabled, "and the critical one");
    let t = drawn(&data);
    assert!((t.drawn - 2350.0).abs() < 1.0, "metered to the watt-hour: {} Wh", t.drawn);
    assert_eq!(t.returned, 0.0);
    let s = status(&data);
    assert!(s.ship.fed && (s.ship.drawn_w - 2350.0).abs() < 0.01, "{:?}", s.ship);
    assert!((s.ship.drawn_wh - t.drawn).abs() < 1e-9, "the card reads the ledger");
    assert!((s.generation - 150.0).abs() < 1e-3, "generation stays what the home makes: {}", s.generation);
}

/// REALISTIC MODE RUNS ON ITS OWN GENERATION AND BATTERIES, AS BEFORE, and the
/// same holds in the default mode on an island with no tap. With nothing of its
/// own at night, a 2,500 W load and a 1,250 Wh bank, the bank carries the load for
/// half an hour, then it is shed, and nothing is drawn from the ship. Seen
/// red with the feed ignoring the mode (`feed_watts` returning the reactor in
/// Realistic: the load stayed on past the flat bank).
#[test]
fn realistic_mode_runs_on_its_own_generation_and_batteries() {
    for (realistic, tapped) in [(true, true), (false, false)] {
        let data = store(realistic);
        let mut world = hecs::World::new();
        if tapped {
            spawn_feed_taps(&mut world, PLAYER_HOME, [0]);
        }
        world.spawn((PowerGenerator { output_watts: 0.0, fuel_per_second: 0.0, active: true }, PowerCircuit { island: 0 }));
        let load = world.spawn((PowerConsumer { draw_watts: 2500.0, priority: 2, enabled: true }, PowerCircuit { island: 0 }));
        let bank = world.spawn((
            Battery { charge_wh: 1250.0, capacity_wh: 4000.0, max_charge_w: 5000.0, max_discharge_w: 5000.0 },
            PowerCircuit { island: 0 },
        ));
        let mut sys = ElectricalSystem::new(std::path::Path::new("data"));
        for _ in 0..1700 {
            sys.tick(&mut world, 1.0, &data);
        }
        assert!(world.get::<&PowerConsumer>(load).unwrap().enabled, "realistic {realistic}: the bank carries it");
        for _ in 0..200 {
            sys.tick(&mut world, 1.0, &data);
        }
        assert!(!world.get::<&PowerConsumer>(load).unwrap().enabled, "realistic {realistic}: shed once the bank is flat");
        assert!(world.get::<&Battery>(bank).unwrap().charge_wh < 1.0);
        assert_eq!(drawn(&data), SupplyTally::default(), "realistic {realistic}: nothing from the ship");
        assert!(!status(&data).ship.fed);
    }
}

/// OWN GENERATION OFFSETS THE REACTOR ONE FOR ONE. A fed 1,000 W load draws
/// 1,000 W from the reactor with nothing of its own, 700 W beside 300 W of
/// panels, and nothing beside 1,300 W, which sends 300 W back to the ship. A
/// SOLAR PANEL BUILT from the blueprint joins the fed home island and takes
/// its 400 W off the draw. Seen red with the feed left out of the island's
/// supply (nothing was drawn), and with the blueprint's `generates` dropped
/// (the built panel was never wired).
#[test]
fn own_solar_offsets_reactor_draw_one_for_one() {
    use crate::systems::construction::{wire_built_generators, BlueprintRegistry, Structure};
    let data = store(false);
    let mut sys = ElectricalSystem::new(std::path::Path::new("data"));
    for (own, want_drawn, want_back) in [(0.0, 1000.0, 0.0), (300.0, 700.0, 0.0), (1300.0, 0.0, 300.0)] {
        let mut world = hecs::World::new();
        spawn_feed_taps(&mut world, PLAYER_HOME, [0]);
        world.spawn((PowerGenerator { output_watts: own, fuel_per_second: 0.0, active: true }, PowerCircuit { island: 0 }));
        world.spawn((PowerConsumer { draw_watts: 1000.0, priority: 2, enabled: true }, PowerCircuit { island: 0 }));
        sys.tick(&mut world, 1.0, &data);
        let s = status(&data).ship;
        assert!((s.drawn_w - want_drawn).abs() < 1e-3 && (s.returned_w - want_back).abs() < 1e-3, "{own} W own: {s:?}");
    }
    // The buildable panel.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("blueprints").join("basic.ron");
    let reg = BlueprintRegistry::from_ron(&std::fs::read(path).unwrap()).unwrap();
    let mut world = hecs::World::new();
    spawn_feed_taps(&mut world, PLAYER_HOME, [0]);
    world.spawn((PowerGenerator { output_watts: 0.0, fuel_per_second: 0.0, active: true }, PowerCircuit { island: 0 }));
    world.spawn((PowerConsumer { draw_watts: 1000.0, priority: 2, enabled: true }, PowerCircuit { island: 0 }));
    let panel = world.spawn((Structure { blueprint_id: "solar_panel".into(), health: 1.0, max_health: 1.0, provides: None, uid: 0 },));
    wire_built_generators(&mut world, &reg);
    assert_eq!(world.get::<&PowerCircuit>(panel).unwrap().island, 0, "on the home's fed island");
    sys.tick(&mut world, 1.0, &data);
    assert!((status(&data).ship.drawn_w - 600.0).abs() < 1e-3, "{:?}", status(&data).ship);
}

/// A PLANET SITE IS NOT FED BY THE SHIP'S REACTOR. A panel built at a site on
/// Earth gets the site's own island, and the stove built beside it joins that
/// island, not the home's; at night (its panel at 0 W) the stove's 1,200 W is
/// shed even in the Station-supplied mode, while the same load aboard stays
/// on, and the ledger holds only the home's draw. Seen red with the site's
/// island numbered 0, the home's fed island (`site_island` returning 0: the
/// island check failed), and with the feed left out of the island's supply
/// (the load aboard was shed).
#[test]
fn a_planet_site_is_not_fed_by_the_ships_reactor() {
    use crate::systems::construction::{wire_built_generators, wire_built_stations, BlueprintRegistry, PlanetSite, Structure};
    let data = store(false);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("blueprints").join("basic.ron");
    let reg = BlueprintRegistry::from_ron(&std::fs::read(path).unwrap()).unwrap();
    let site = PlanetSite { body: "earth".into(), origin: glam::DVec3::new(0.0, 6_371_000.0, 0.0) };
    let mut world = hecs::World::new();
    spawn_feed_taps(&mut world, PLAYER_HOME, [0]);
    world.spawn((PowerGenerator { output_watts: 0.0, fuel_per_second: 0.0, active: true }, PowerCircuit { island: 0 }));
    let aboard = world.spawn((PowerConsumer { draw_watts: 1200.0, priority: 2, enabled: true }, PowerCircuit { island: 0 }));
    let s = |id: &str| Structure { blueprint_id: id.into(), health: 1.0, max_health: 1.0, provides: None, uid: 0 };
    let panel = world.spawn((s("solar_panel"), site.clone()));
    let stove = world.spawn((s("stove"), site.clone()));
    wire_built_generators(&mut world, &reg);
    wire_built_stations(&mut world, &reg);
    let island = world.get::<&PowerCircuit>(panel).unwrap().island;
    assert_eq!(island, site_island(&site));
    assert_ne!(island, 0, "not the home's island");
    assert_eq!(world.get::<&PowerCircuit>(stove).unwrap().island, island, "the site's stove joins the site's panel");
    // Night at the site, and the stove working.
    world.get::<&mut PowerGenerator>(panel).unwrap().output_watts = 0.0;
    world.get::<&mut PowerConsumer>(stove).unwrap().draw_watts = 1200.0;
    let mut sys = ElectricalSystem::new(std::path::Path::new("data"));
    for _ in 0..3600 {
        sys.tick(&mut world, 1.0, &data);
    }
    assert!(!world.get::<&PowerConsumer>(stove).unwrap().enabled, "no reactor at the site: shed");
    assert!(world.get::<&PowerConsumer>(aboard).unwrap().enabled, "aboard, the reactor carries it");
    assert!((drawn(&data).drawn - 1200.0).abs() < 1.0, "only the home's draw is metered: {:?}", drawn(&data));
}

/// The ledger is shaped for a fleet: homes kept apart, and summed per utility.
#[test]
fn the_fleet_total_sums_every_homes_lines() {
    let mut l = ShipSupplyLedger::default();
    l.record("home", POWER, 100.0, 5.0);
    l.record("home", POWER, 50.0, 0.0);
    l.record("neighbour", POWER, 30.0, 1.0);
    l.record("neighbour", "water", 12.0, 0.0);
    assert_eq!(l.tally("home", POWER), SupplyTally { drawn: 150.0, returned: 5.0 });
    let f = l.fleet_total();
    assert_eq!(f[POWER], SupplyTally { drawn: 180.0, returned: 6.0 });
    assert_eq!(f["water"].drawn, 12.0);
}

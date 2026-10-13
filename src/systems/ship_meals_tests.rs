//! Tests of the mess hall's counter on this computer (ship_meals.rs). Each was seen to fail
//! before it passed; the failure text is in its comment.

use super::*;
use crate::ecs::components::{Controllable, Health, StatusEffects, Vitals};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::food::FoodSystem;
use std::sync::Mutex;

fn data_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// The DataStore as lib.rs wires it for the FoodSystem, with the quest events channel the
/// opening's Eat step reads.
fn store() -> DataStore {
    let mut data = DataStore::new();
    data.insert(
        "status_effect_registry",
        crate::systems::status_effects::StatusEffectRegistry::from_csv(include_bytes!("../../data/status_effects.csv")).expect("status_effects.csv"),
    );
    for slot in ["consume_request", "drink_request", "use_item_request"] {
        data.insert(slot, Mutex::new(Option::<String>::None));
    }
    data.insert("rest_request", Mutex::new(false));
    data.insert("compost_request", Mutex::new(false));
    data.insert("player_death", Mutex::new(Option::<String>::None));
    data.insert("player_notices", Mutex::new(Vec::<String>::new()));
    data.insert("quest_events", Mutex::new(Vec::<String>::new()));
    let mut clock = crate::systems::time::GameTime::default();
    clock.time_scale = 1.0;
    data.insert("game_time", Mutex::new(clock));
    data
}

/// A hungry player: satiation 40 of 100, everything else comfortable.
fn hungry(world: &mut hecs::World) -> hecs::Entity {
    let vitals = Vitals { satiation: 40.0, hydration: 100.0, energy: 100.0, body_temp_c: crate::systems::body_heat::CORE_NEUTRAL_C as f32, ..Vitals::default() };
    world.spawn((Controllable, Inventory::new(8), vitals, StatusEffects::default(), Health::default()))
}

/// THE COUNTER SERVES A MEAL OFFLINE, ONCE A MEAL TIME (decision 4 of the increment 6 plan:
/// meals any time, one a meal time per person). The first meal is served, counted on the meter
/// (taken 1, the next a meal time on) and, eaten the way the counter has it eaten (into the
/// backpack, then the Eat channel the food system drains), it fills the stomach and reports the
/// Eat event the Cabin's opening counts. A second a minute later is not yet, and says when in
/// words; one a meal time later is served. A meter whose next meal is further off than one meal
/// time (the clock went back) serves at once.
///
/// Seen red 2026-10-12 with `serve`'s interval check taken out (every ask served): "a second
/// meal a minute later is not yet / left: Meal { item: \"ration_basic_0\" } / right: NotYet {
/// ready_in_s: 28740.0 }".
#[test]
fn the_counter_serves_a_meal_offline_once_a_meal_time() {
    let rules = MealRules::load(&data_dir());
    assert_eq!(rules, MealRules { interval_s: 8.0 * 3600.0, item: "ration_basic_0".into() }, "the server's own meal time and meal");
    let mut world = hecs::World::new();
    let me = hungry(&mut world);
    let data = store();
    let mut food = FoodSystem::new(&data_dir());
    let mut meter = ShipMeals::default();

    let first = serve(&mut meter, 1000.0, &rules);
    assert_eq!(first, Served::Meal { item: "ration_basic_0".into() });
    assert_eq!((meter.taken, meter.next_at_game_s), (1, 1000.0 + 28_800.0), "taken 1, the next a meal time on");
    world.get::<&mut Inventory>(me).unwrap().add_item("ration_basic_0", 1, 99);
    *data.get::<Mutex<Option<String>>>("consume_request").unwrap().lock().unwrap() = Some("ration_basic_0".into());
    food.tick(&mut world, 0.0, &data);
    let satiation = world.get::<&Vitals>(me).unwrap().satiation;
    assert!(satiation > 40.0, "the meal is eaten and the stomach fuller: satiation {satiation}");
    assert_eq!(world.get::<&Inventory>(me).unwrap().count_item("ration_basic_0"), 0, "eaten, not carried");
    let events = data.get::<Mutex<Vec<String>>>("quest_events").unwrap().lock().unwrap().clone();
    assert!(events.contains(&crate::systems::quests::eat_event_key("ration_basic_0")), "the opening's Eat step hears it: {events:?}");

    let second = serve(&mut meter, 1060.0, &rules);
    assert_eq!(second, Served::NotYet { ready_in_s: 28_740.0 }, "a second meal a minute later is not yet / left: {second:?} / right: NotYet {{ ready_in_s: 28740.0 }}");
    assert_eq!(meter.taken, 1, "nothing counted for it");
    assert_eq!(not_yet_sentence(28_740.0), "Your next crew meal is ready in 7 h 59 min.");
    assert_eq!(not_yet_sentence(90.0), "Your next crew meal is ready in 2 min.");

    let third = serve(&mut meter, 1000.0 + 28_800.0, &rules);
    assert_eq!(third, Served::Meal { item: "ration_basic_0".into() }, "a meal time later it is served");
    assert_eq!(meter.taken, 2);

    // The clock went back (an older save, a fresh world each launch): no wait past one meal time.
    let mut back = ShipMeals { taken: 5, next_at_game_s: 90_000.0, water_l: 0.0 };
    assert!(matches!(serve(&mut back, 100.0, &rules), Served::Meal { .. }), "a clock that went back never makes the wait longer than a meal time");
}

/// FILLING THE BACKPACK'S VESSELS is a swap, every empty water vessel at once, and a full one
/// that does not fit goes back empty rather than being dropped; the sentence counts them.
///
/// Seen red 2026-10-12 with the fill looking for the jerrycan only (`f.litres > 1.0` added to
/// the filter): "both bottles are full / left: 0 / right: 2".
#[test]
fn every_empty_water_vessel_is_filled_by_a_swap() {
    let root = data_dir();
    let table = FluidTable::load(&root);
    let reg = ItemRegistry::from_csv(&std::fs::read(root.join("items.csv")).unwrap()).unwrap();
    let mut world = hecs::World::new();
    let mut inv = Inventory::new(12);
    inv.add_item("water_bottle_empty_0", 2, 99);
    inv.add_item("water_jerrycan_empty_0", 1, 99);
    inv.add_item("bread_0", 1, 99);
    let me = world.spawn((inv, Controllable));
    let filled = fill_vessels(&table, &mut world, Some(&reg));
    let inv = world.get::<&Inventory>(me).unwrap();
    assert_eq!(inv.count_item("water_bottle_0"), 2, "both bottles are full / left: {} / right: 2", inv.count_item("water_bottle_0"));
    assert_eq!((inv.count_item("water_jerrycan_0"), inv.count_item("water_bottle_empty_0"), inv.count_item("bread_0")), (1, 0, 1));
    assert_eq!(filled.count(), 3);
    assert!((filled.litres - 21.0).abs() < 1e-9, "{}", filled.litres);
    assert_eq!(filled.sentence(), "Filled 2 x Water Bottle, 1 x Water Jerrycan (21 L) from the ship's water.");
    drop(inv);
    assert_eq!(fill_vessels(&table, &mut world, Some(&reg)), Filled::default(), "nothing empty left to fill");
    assert_eq!(Filled::default().sentence(), "You carry no empty bottle to fill.");
}

/// THE BUILT-IN RULES ARE THE SERVER'S: the counter reads the same two files the relay does, and
/// both read them the same (the meal time from ship_stores.ron, the meal from the ledger's meal
/// kind), so a meal on this computer is the meal a server would serve.
///
/// Seen red 2026-10-12 with the meal kind looked up as "meals": "the counter reads the ledger's
/// meal: Err(\"ship/fleet_ledger.ron: no meal kind\")".
#[cfg(feature = "relay")]
#[test]
fn the_counters_rules_are_the_servers() {
    let stores = include_str!("../../data/food/ship_stores.ron");
    let ledger = include_str!("../../data/ship/fleet_ledger.ron");
    let rules = MealRules::parse(stores, ledger);
    assert!(rules.is_ok(), "the counter reads the ledger's meal: {rules:?}");
    let rules = rules.unwrap();
    let relay_stores = crate::relay::handlers::ship_stores::Provisions::parse(stores).expect("the relay reads it");
    let relay_ledger = crate::relay::handlers::fleet_ledger::LedgerData { file: crate::relay::handlers::fleet_ledger::LedgerFile::parse(ledger).unwrap(), goods: Default::default() };
    assert_eq!(rules.interval_s, relay_stores.meal_interval_hours * 3600.0);
    assert_eq!(Some(rules.item.as_str()), relay_ledger.meal_item());
    assert_eq!(MealRules::built_in(), rules);
    assert!(MealRules::parse("(meal_interval_hours: 0.0)", ledger).is_err(), "a meal time of 0 is refused");
}

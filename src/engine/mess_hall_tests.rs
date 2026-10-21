//! Tests of the mess hall's serving counter (mess_hall.rs). Each was seen to fail before it
//! passed; the failure text is in its comment.

use super::*;
use crate::gui::pages::fleet_ledger::FleetStore;
use crate::persistence::ShipMeals;
use crate::systems::inventory::Inventory;
use std::sync::Mutex;

fn data_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// A DataStore with the meter registered (startup's job, Wave 2A; the tests do it themselves)
/// and the world's clock at `now` game seconds.
fn store(now: f64) -> DataStore {
    let mut data = DataStore::new();
    crate::save_load::register_ship_meals(&mut data);
    let mut clock = crate::systems::time::GameTime::default();
    clock.elapsed_seconds = now;
    data.insert("game_time", Mutex::new(clock));
    data.insert("item_registry", crate::systems::inventory::ItemRegistry::from_csv(&std::fs::read(data_dir().join("items.csv")).unwrap()).unwrap());
    data
}

/// The player, carrying `pack`.
fn world_with(pack: &[(&str, u32)]) -> (hecs::World, hecs::Entity) {
    let mut world = hecs::World::new();
    let mut inv = Inventory::new(12);
    for (id, n) in pack {
        inv.add_item(id, *n, 99);
    }
    let p = world.spawn((inv, crate::ecs::components::Controllable));
    (world, p)
}

fn count(world: &hecs::World, p: hecs::Entity, id: &str) -> u32 {
    world.get::<&Inventory>(p).unwrap().count_item(id)
}

/// The meter as the DataStore holds it.
fn meter_of(data: &DataStore) -> ShipMeals {
    crate::save_load::ship_meals(data).expect("registered")
}

/// IN THE SHARED WORLD THE COUNTER ASKS THE SERVER: Take a meal sends ONE `game_interact
/// take_meal` to the nearest fleet store in reach (of two, the near one), and serves nothing of
/// its own: no ration in the backpack, nothing to eat, nothing counted on this computer. With no
/// store in reach it says to walk closer and sends nothing; and still no meal of its own, though
/// the player stands right at the counter (the meal must be the server's, on the ledger).
///
/// Seen red 2026-10-12 with `take_meal_at` taking the offline path while joined (its
/// `copresence_active` branch removed): "a meal not on the ledger: this computer counted 1 meal
/// while joined / left: 1 / right: 0".
#[test]
fn in_the_shared_world_the_counter_asks_the_server() {
    let (mut world, p) = world_with(&[]);
    let mut data = store(10_000.0);
    let dir = data_dir();
    let mut gs = GuiState::default();
    gs.copresence_active = true;
    let (client, sent) = crate::net::ws_client::WsClient::recording();
    gs.ws_client = Some(client);
    gs.fleet.stores = vec![
        FleetStore { entity_id: 12, name: "The mess hall's stores".into(), position: [67.0, 1.0, 22.0] },
        FleetStore { entity_id: 40, name: "The South Commons' stores".into(), position: [79.0, 1.0, 772.0] },
    ];
    gs.fleet.my_position = Some([67.5, 1.7, 23.0]);

    let meal = take_meal_at(&mut Counter { gs: &mut gs, world: &mut world, data: &mut data, data_dir: &dir, counter_m: Some(1.0) });
    assert_eq!(meter_of(&data).taken, 0, "a meal not on the ledger: this computer counted {} meal while joined / left: {} / right: 0", meter_of(&data).taken, meter_of(&data).taken);
    assert_eq!((count(&world, p, "ration_basic_0"), gs.pending_consume_item.clone()), (0, None), "nothing of its own to eat");
    let frames: Vec<serde_json::Value> = sent.try_iter().map(|f| serde_json::from_str(&f).unwrap()).collect();
    assert_eq!(frames.len(), 1, "one request: {frames:?}");
    assert_eq!(frames[0], serde_json::json!({ "type": "game_interact", "entity_id": 12, "action": "take_meal" }), "to the store in reach");
    assert_eq!(meal, Meal::Asked(frames[0].clone()));
    assert_eq!(meal.sentence(), "Asking the ship's stores for a meal...");

    // Out of every store's reach: the walk-closer sentence, nothing sent, still nothing served.
    gs.fleet.my_position = Some([67.0, 1.7, 40.0]);
    let meal = take_meal_at(&mut Counter { gs: &mut gs, world: &mut world, data: &mut data, data_dir: &dir, counter_m: Some(1.0) });
    assert_eq!(meal, Meal::No("Walk within 5 m of The mess hall's stores to take a meal (you are 18 m away).".into()));
    assert_eq!(sent.try_iter().count(), 0, "nothing sent");
    assert_eq!(meter_of(&data).taken, 0, "a meal not on the ledger");
    assert_eq!(gs.pending_consume_item, None);
}

/// AWAY FROM A SERVER THE COUNTER SERVES THE SHIP'S STORES HERE: at the counter, the meal goes
/// into the backpack and is asked to be eaten through `fleet::feed` (the Eat button's path, so
/// the food system eats it and the Cabin's opening hears it), counted on the meter; a second
/// ask is not yet, in words; from across the room it says to walk up first.
///
/// Seen red 2026-10-12 with the meal counted but not fed (the `feed` call skipped): "the meal
/// is eaten through the Eat path / left: None / right: Some(\"ration_basic_0\")".
#[test]
fn away_from_a_server_the_counter_serves_the_ships_stores_here() {
    let (mut world, p) = world_with(&[]);
    let mut data = store(10_000.0);
    let dir = data_dir();
    let mut gs = GuiState::default();

    let far = take_meal_at(&mut Counter { gs: &mut gs, world: &mut world, data: &mut data, data_dir: &dir, counter_m: Some(12.0) });
    assert_eq!(far, Meal::No(WALK_TO_COUNTER.into()));
    assert_eq!(meter_of(&data).taken, 0);

    let meal = take_meal_at(&mut Counter { gs: &mut gs, world: &mut world, data: &mut data, data_dir: &dir, counter_m: Some(1.0) });
    assert_eq!(meal, Meal::Served("ration_basic_0".into()));
    assert_eq!(gs.pending_consume_item.as_deref(), Some("ration_basic_0"), "the meal is eaten through the Eat path / left: {:?} / right: Some(\"ration_basic_0\")", gs.pending_consume_item);
    assert_eq!(count(&world, p, "ration_basic_0"), 1, "from the backpack, where the Eat path takes it");
    assert_eq!(meter_of(&data), ShipMeals { taken: 1, next_at_game_s: 10_000.0 + 28_800.0, water_l: 0.0 });

    gs.pending_consume_item = None;
    let again = take_meal_at(&mut Counter { gs: &mut gs, world: &mut world, data: &mut data, data_dir: &dir, counter_m: Some(1.0) });
    assert_eq!(again.sentence(), "Your next crew meal is ready in 8 h 0 min.");
    assert_eq!((meter_of(&data).taken, gs.pending_consume_item.clone()), (1, None), "one a meal time");
}

/// FILLING BOTTLES AT THE COUNTER fills every empty water bottle the backpack holds from the
/// ship's water, counts the litres on the meter and says how many; the same in a server's world;
/// with nothing empty it fills nothing and says so; from across the room it says to walk up.
///
/// Seen red 2026-10-12 with `fill_bottles_at` returning before the fill (nothing filled):
/// "both bottles are full / left: 0 / right: 2".
#[test]
fn filling_bottles_at_the_counter() {
    let (mut world, p) = world_with(&[("water_bottle_empty_0", 2), ("bread_0", 1)]);
    let mut data = store(0.0);
    let dir = data_dir();
    let mut gs = GuiState::default();

    let said = fill_bottles_at(&mut Counter { gs: &mut gs, world: &mut world, data: &mut data, data_dir: &dir, counter_m: Some(1.5) });
    assert_eq!(count(&world, p, "water_bottle_0"), 2, "both bottles are full / left: {} / right: 2", count(&world, p, "water_bottle_0"));
    assert_eq!(count(&world, p, "water_bottle_empty_0"), 0);
    assert_eq!(said, "Filled 2 x Water Bottle (1 L) from the ship's water.");
    assert!((meter_of(&data).water_l - 1.0).abs() < 1e-9, "the litres are counted: {:?}", meter_of(&data));

    let said = fill_bottles_at(&mut Counter { gs: &mut gs, world: &mut world, data: &mut data, data_dir: &dir, counter_m: Some(1.5) });
    assert_eq!(said, "You carry no empty bottle to fill.");
    assert!((meter_of(&data).water_l - 1.0).abs() < 1e-9, "nothing more counted");

    // In a server's world, the same.
    gs.copresence_active = true;
    world.get::<&mut Inventory>(p).unwrap().add_item("water_bottle_empty_0", 1, 99);
    let far = fill_bottles_at(&mut Counter { gs: &mut gs, world: &mut world, data: &mut data, data_dir: &dir, counter_m: Some(9.0) });
    assert_eq!((far.as_str(), count(&world, p, "water_bottle_empty_0")), (WALK_TO_COUNTER, 1), "from across the room nothing is filled");
    let said = fill_bottles_at(&mut Counter { gs: &mut gs, world: &mut world, data: &mut data, data_dir: &dir, counter_m: Some(1.5) });
    assert_eq!(said, "Filled 1 x Water Bottle (0.5 L) from the ship's water.");
    assert!((meter_of(&data).water_l - 1.5).abs() < 1e-9);
}

/// ONLY A SERVING COUNTER OFFERS A MEAL: the family home's machines with the ship's merged in,
/// plus one serving counter row (this test adds it the way a ship row is written: zone
/// "commons", type `serving_counter`; lane 1A puts the real ones in data/machines/ship.ron). Its
/// card offers a meal; no other row's does, every Furniture row (tables, chairs, couches, beds,
/// shelves) included; no card pinned, or no machines, offers none. The nearest counter is
/// measured from its own label only.
///
/// Seen red 2026-10-12 with `card_offers_meal` true for any row whose type is Furniture: "the
/// shelf row shelf_1 offers no meal".
#[test]
fn only_a_serving_counter_offers_a_meal() {
    let mut home = crate::machines::MachineHome::load(&data_dir().join("machines/home.ron")).expect("home.ron loads");
    let mut counter = home.instances.iter().find(|i| i.zone != "home").cloned().expect("a ship row to copy the shape of");
    counter.id = "mess-hall-counter".into();
    counter.machine = COUNTER_TYPE.into();
    counter.zone = "commons".into();
    home.instances.push(counter);
    assert!(card_offers_meal(Some(&home), Some("mess-hall-counter")), "the counter's card offers a meal");
    let furniture = home.instances.iter().filter(|i| home.catalog.get(&i.machine).is_some_and(|d| d.category == "Furniture")).count();
    assert!(furniture >= 3, "the home has furniture to check against: {furniture}");
    for i in home.instances.iter().filter(|i| i.machine != COUNTER_TYPE) {
        assert!(!card_offers_meal(Some(&home), Some(&i.id)), "the {} row {} offers no meal", i.machine, i.id);
    }
    assert!(!card_offers_meal(Some(&home), None), "no card pinned");
    assert!(!card_offers_meal(None, Some("mess-hall-counter")), "no machines");

    let label = |id: &str, x: f32| crate::gui::MachineLabel {
        pos: glam::Vec3::new(x, 1.2, 0.0),
        name: id.into(),
        stats: Vec::new(),
        room: String::new(),
        machine_id: id.into(),
        on_use: None,
    };
    let other = home.instances.iter().find(|i| i.machine != COUNTER_TYPE).unwrap().id.clone();
    let labels = vec![label(&other, 0.5), label("mess-hall-counter", 4.0)];
    let d = nearest_counter_m(Some(&home), &labels, glam::Vec3::new(0.0, 1.2, 0.0)).expect("a counter");
    assert!((d - 4.0).abs() < 1e-5, "measured from the counter, not the nearer {other}: {d}");
    assert_eq!(nearest_counter_m(Some(&home), &labels[..1], glam::Vec3::ZERO), None);

    // A COUNTER ONE STOREY UP IS NOT YOURS (ship homes increment 7): its label 1.3 m over a floor
    // 3.0 m up (0.9 m counter, 0.4 m over it), right over a player whose eyes are 1.7 m up, is out
    // of reach though 2.6 m away; on the player's floor, 1 m away across the floor, it is in reach;
    // and from upstairs the counter under the feet is not. Seen red 2026-10-20 on the reach before
    // storeys (the distance alone): "a counter one storey up was the nearest: Some(2.6)".
    let at = |y: f32| vec![crate::gui::MachineLabel { pos: glam::Vec3::new(1.0, y, 0.0), ..label("mess-hall-counter", 1.0) }];
    let eye = glam::Vec3::new(1.0, 1.7, 0.0);
    let up = nearest_counter_m(Some(&home), &at(3.0 + 1.3), eye);
    assert_eq!(up, None, "a counter one storey up was the nearest: {up:?}");
    let here = nearest_counter_m(Some(&home), &at(1.3), eye + glam::Vec3::new(1.0, 0.0, 0.0));
    assert!(here.is_some_and(|d| d <= INTERACT_REACH_M), "a counter on your floor: {here:?}");
    assert_eq!(nearest_counter_m(Some(&home), &at(1.3), eye + glam::Vec3::new(0.0, 3.0, 0.0)), None, "the counter under an upstairs player");
}

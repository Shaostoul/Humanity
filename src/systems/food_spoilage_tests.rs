//! Food spoils wherever it is kept (first-hour audit S6, docs/design/first-hour-audit-2026-10-04.md):
//! in the backpack, in home storage (the Barn, the home's other places, a built chest) and in
//! the home's vessels, each by its own clock that survives moving it and a save, faster or
//! slower by the temperature zone where it is kept (data/food_system.ron).
//!
//! A child of `food` (`use super::*`), kept in its own file so food.rs does not grow into a
//! monolith.
//!
//! RED CHECKS. Each "Seen red" below was run on 2026-10-04 on 1df4c0340 against this change's
//! skeleton: the new fields and calls in place, doing what the code did before, i.e. food
//! aged only in the FoodSystem's side table keyed by backpack slot, no stack, stored item or
//! vessel had a clock of its own, every temperature counted as room temperature, and no move
//! or save carried an age.

use super::*;
use crate::ecs::components::{Health, StatusEffects, Vitals};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::inventory::containers::{Container, ContainerRegistry};
use crate::systems::inventory::placed::PlacedItem;
use crate::systems::inventory::{Inventory, ItemStack, TransferOp};
use std::sync::Mutex;

fn data_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/data"))
}

/// The DataStore as lib.rs wires it for the FoodSystem, with the needs held still (the Vitals
/// drain slider at 0), the shipped container types, and the slot home storage's aging rides in.
fn store() -> DataStore {
    let mut data = DataStore::new();
    let reg = crate::systems::status_effects::StatusEffectRegistry::from_csv(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/status_effects.csv"
    )))
    .expect("status_effects.csv");
    data.insert("status_effect_registry", reg);
    for slot in ["consume_request", "drink_request"] {
        data.insert(slot, Mutex::new(Option::<String>::None));
    }
    data.insert("rest_request", Mutex::new(false));
    data.insert("compost_request", Mutex::new(false));
    data.insert("player_death", Mutex::new(Option::<String>::None));
    data.insert("vitals_drain_scale", Mutex::new(0.0_f32));
    data.insert(STORAGE_AGING_KEY, Mutex::new(0.0_f64));
    let root = data_dir().join("containers");
    let containers = ContainerRegistry::from_bytes(
        &std::fs::read(root.join("types.csv")).unwrap(),
        &std::fs::read(root.join("content_classes.ron")).unwrap(),
    )
    .unwrap();
    data.insert("container_registry", containers);
    data
}

fn player(world: &mut hecs::World, inv: Inventory) -> hecs::Entity {
    world.spawn((
        crate::ecs::components::Controllable,
        inv,
        Vitals::default(),
        StatusEffects::default(),
        Health::default(),
    ))
}

/// A home vessel of the shipped type `container_type`, holding `qty` of `item`.
fn vessel(world: &mut hecs::World, container_type: &str, item: &str, qty: u32) -> hecs::Entity {
    let mut c = Container::new(container_type, 400.0);
    c.current_content_item = Some(item.to_string());
    c.last_content = Some(item.to_string());
    c.current_qty = qty;
    c.used_liters = qty as f32;
    world.spawn((c,))
}

fn barn(key: &str, qty: u32) -> PlacedItem {
    PlacedItem { key: key.into(), name: key.into(), qty, container: "1/1".into(), ..Default::default() }
}

/// What the frame does with home storage's share after the tick
/// (`engine::stock_piles::age_home_storage`), without an engine.
fn age_storage(pool: &mut [PlacedItem], data: &DataStore) {
    let secs = data.get::<Mutex<f64>>(STORAGE_AGING_KEY).unwrap().lock().map(|mut d| std::mem::take(&mut *d)).unwrap();
    let food = consume_kinds();
    crate::systems::inventory::placed::age_food(pool, secs, |k| food.contains_key(k));
}

fn age_of(world: &hecs::World, p: hecs::Entity, item: &str) -> f64 {
    world.get::<&Inventory>(p).unwrap().slots.iter().flatten().find(|s| s.item_id == item).map(|s| s.age_s).unwrap()
}

/// The multiplier a temperature zone of data/food_system.ron gives the clock.
fn zone(sys: &FoodSystem, id: &str) -> f64 {
    f64::from(sys.data.temperature_zones.iter().find(|z| z.id == id).expect("the zone is in the data").spoilage_rate_multiplier)
}

/// FOOD AGES WHEREVER IT IS KEPT (first-hour audit S6, 2026-10-04): in the backpack, in a
/// vessel of the home (the pantry cabinet) and in home storage (the Barn), each by its own
/// clock, an hour for an hour at the room temperature of the home's air (the zones' 1.0).
/// What is not food does not age. Only an inventory on an entity used to age, so food in
/// storage never spoiled.
///
/// Seen red (see the module doc): "bread in the backpack aged an hour / left: 0.0 / right:
/// 3600.0".
#[test]
fn food_ages_wherever_it_is_kept() {
    let mut sys = FoodSystem::new(data_dir());
    let data = store();
    let mut world = hecs::World::new();
    let mut inv = Inventory::new(8);
    inv.add_item("bread_0", 2, 99);
    inv.add_item("rope_0", 1, 99);
    let p = player(&mut world, inv);
    let pantry = vessel(&mut world, "pantry_cabinet_bin", "bread_0", 4);
    let mut pool = vec![barn("bread_0", 10), barn("rope_0", 3)];

    sys.tick(&mut world, 3600.0, &data);
    age_storage(&mut pool, &data);
    assert_eq!(age_of(&world, p, "bread_0"), 3600.0, "bread in the backpack aged an hour");
    assert_eq!(world.get::<&Container>(pantry).unwrap().content_age_s, 3600.0, "bread in the pantry aged an hour");
    assert_eq!(pool[0].age_s, 3600.0, "bread in the Barn aged an hour");
    assert_eq!(age_of(&world, p, "rope_0"), 0.0, "rope is not food");
    assert_eq!(pool[1].age_s, 0.0, "rope in the Barn is not food");
}

/// A COLD STORE AGES FOOD SLOWER, AS THE DATA SAYS (first-hour audit S6): the freezer keeps
/// its food in the "frozen" zone of data/food_system.ron (data/containers/types.csv
/// keeps_zone), so an hour in it is that zone's share of an hour; a backpack carried in
/// air at 4 C ages by the "cold" zone; and the home's air at room temperature is the
/// baseline. The multipliers are read from the data here, never written into the test.
///
/// Seen red (see the module doc): "an hour in the freezer / left: 0 / right:
/// 180.00000268220901" (the data's 0.05, read as f32).
#[test]
fn a_cold_store_ages_food_slower_as_the_data_says() {
    let mut sys = FoodSystem::new(data_dir());
    let mut data = store();
    let mut world = hecs::World::new();
    let mut inv = Inventory::new(8);
    inv.add_item("bread_0", 1, 99);
    let p = player(&mut world, inv);
    let freezer = vessel(&mut world, "freezer_chest", "roast_chicken_0", 3);
    let pantry = vessel(&mut world, "pantry_cabinet_bin", "bread_0", 3);
    data.insert(
        "environment_context",
        crate::ecs::components::EnvironmentContext { ambient_temp_c: 4.0, ..Default::default() },
    );
    sys.tick(&mut world, 3600.0, &data);
    let (frozen, cold) = (zone(&sys, "frozen"), zone(&sys, "cold"));
    assert!(frozen < cold && cold < 1.0, "the data's zones: frozen {frozen}, cold {cold}");
    let in_freezer = world.get::<&Container>(freezer).unwrap().content_age_s;
    assert!((in_freezer - 3600.0 * frozen).abs() < 1e-6, "an hour in the freezer / left: {in_freezer} / right: {}", 3600.0 * frozen);
    let carried = age_of(&world, p, "bread_0");
    assert!((carried - 3600.0 * cold).abs() < 1e-6, "an hour carried in air at 4 C / left: {carried} / right: {}", 3600.0 * cold);
    assert_eq!(world.get::<&Container>(pantry).unwrap().content_age_s, 3600.0, "the pantry is the home's air");
}

/// The air's temperature picks its zone from the data's own ranges (first-hour audit S6):
/// 21 C is room temperature, 4 C the refrigerator's "cold", -18 C "frozen", and beyond the
/// table its nearest end. The multipliers come from data/food_system.ron.
///
/// Seen red (see the module doc): "4 C is the cold zone / left: 1.0 / right: 0.25".
#[test]
fn the_air_temperature_picks_the_zone() {
    let sys = FoodSystem::new(data_dir());
    let at = |c: f32| sys.data.spoilage_multiplier_at(c);
    assert_eq!(at(21.0), zone(&sys, "room_temp"), "21 C is room temperature");
    assert_eq!(at(4.0), zone(&sys, "cold"), "4 C is the cold zone");
    assert_eq!(at(-18.0), zone(&sys, "frozen"), "-18 C is frozen");
    assert_eq!(at(12.0), zone(&sys, "cool"), "12 C is a cellar's cool");
    assert_eq!(at(-80.0), zone(&sys, "frozen"), "colder than the table: its coldest zone");
    assert_eq!(at(90.0), zone(&sys, "hot"), "hotter than the table: its hottest zone");
}

/// Every zone a container type keeps (data/containers/types.csv `keeps_zone`) is one the food
/// data has, so a typo cannot quietly put the freezer back at room temperature (first-hour
/// audit S6). A data guard.
#[test]
fn every_kept_zone_is_a_zone_the_food_data_has() {
    let sys = FoodSystem::new(data_dir());
    let data = store();
    let reg = data.get::<ContainerRegistry>("container_registry").unwrap();
    let kept: Vec<(&str, &str)> =
        reg.types.values().filter_map(|t| t.keeps_zone.as_deref().map(|z| (t.id.as_str(), z))).collect();
    assert!(kept.iter().any(|(id, z)| *id == "freezer_chest" && *z == "frozen"), "the freezer chest keeps its food frozen: {kept:?}");
    for (id, z) in kept {
        assert!(sys.data.zone_multiplier(z).is_some(), "{id} keeps zone {z}, which data/food_system.ron does not define");
    }
}

/// MOVING FOOD KEEPS ITS AGE (first-hour audit S6): out of the Barn into the backpack (the
/// Inventory page's "Take to backpack", a transfer carrying the item's age), back into a
/// vessel and out again (the machine card's Store and Take), and a fresh loaf joining old ones
/// gives the stack their count-weighted average, so no age is made or lost. The spoilage clock
/// used to live in a side table keyed by backpack slot, so any move started it again.
///
/// Seen red (see the module doc): "the Barn's bread arrives as old as it was / left: 0.0 /
/// right: 5000.0".
#[test]
fn moving_food_keeps_its_age() {
    let mut data = store();
    data.insert("inventory_transfer_ops", Mutex::new(Vec::<TransferOp>::new()));
    data.insert("inventory_transfer_returns", Mutex::new(Vec::<(String, u32)>::new()));
    let mut inv_sys = crate::systems::inventory::InventorySystem::new();
    let mut world = hecs::World::new();
    let p = player(&mut world, Inventory::new(8));
    let stored = PlacedItem { age_s: 5000.0, ..barn("bread_0", 3) };
    // Take to backpack: the GUI's op carries the item's age.
    data.get::<Mutex<Vec<TransferOp>>>("inventory_transfer_ops").unwrap().lock().unwrap().push(TransferOp {
        item_id: stored.key.clone(),
        qty: stored.qty,
        add: true,
        wear: stored.wear,
        quality: stored.quality,
        age_s: stored.age_s,
    });
    inv_sys.tick(&mut world, 0.0, &data);
    assert_eq!(age_of(&world, p, "bread_0"), 5000.0, "the Barn's bread arrives as old as it was");

    // A fresh loaf joins the three: the stack is the average, 3 x 5000 / 4.
    world.get::<&mut Inventory>(p).unwrap().add_item("bread_0", 1, 99);
    assert_eq!(age_of(&world, p, "bread_0"), 3750.0, "a fresh loaf takes a share of the old ones' age");

    // Into the pantry and out again (the machine card's Store, then Take).
    let pantry = vessel(&mut world, "pantry_cabinet_bin", "bread_0", 0);
    {
        let (left, age) = world.get::<&mut Inventory>(p).unwrap().remove_item_aged("bread_0", 4);
        assert_eq!((left, age), (0, 3750.0), "the four that left were 3750 s old");
        let reg = data.get::<ContainerRegistry>("container_registry").unwrap();
        let mut c = world.get::<&mut Container>(pantry).unwrap();
        reg.try_store(&mut c, "bread_0", "food", 0.0, 4);
        c.arrived_aged(4, age);
        assert_eq!(c.content_age_s, 3750.0, "the pantry holds them at their age");
    }
    let held = world.get::<&Container>(pantry).unwrap().content_age_s;
    world.get::<&mut Inventory>(p).unwrap().add_item_volume_gated_aged("bread_0", 4, 99, 0.0, 0, held);
    assert_eq!(age_of(&world, p, "bread_0"), 3750.0, "taken out at the age they had in the pantry");

    // Back to storage when the backpack is full (`gui::return_to_storage`), at their age.
    let mut pool = Vec::new();
    let origin = PlacedItem { age_s: 3750.0, ..barn("bread_0", 2) };
    crate::gui::return_to_storage(&mut pool, "bread_0", 2, Some(&origin));
    assert_eq!(pool[0].age_s, 3750.0, "what the backpack could not take goes back as old as it was");
}

/// SPOILED FOOD FROM STORAGE POISONS (first-hour audit S6): bread left in the Barn past its
/// profile's time (data/food_system.ron, bread) is spoiled; taken to the backpack it stays
/// spoiled, and eating it poisons and feeds a quarter, as spoiled food from the backpack
/// always did.
///
/// Seen red (see the module doc): "the bread in the Barn is past its time".
#[test]
fn spoiled_food_from_storage_poisons() {
    let mut sys = FoodSystem::new(data_dir());
    let mut data = store();
    data.insert("inventory_transfer_ops", Mutex::new(Vec::<TransferOp>::new()));
    data.insert("inventory_transfer_returns", Mutex::new(Vec::<(String, u32)>::new()));
    let keeps = sys.freshness_secs("bread_0").expect("bread is food");
    let mut world = hecs::World::new();
    let p = player(&mut world, Inventory::new(8));
    let mut pool = vec![barn("bread_0", 1)];
    // The home's air for longer than bread keeps, in two ticks.
    sys.tick(&mut world, (keeps * 0.6) as f32, &data);
    sys.tick(&mut world, (keeps * 0.6) as f32, &data);
    age_storage(&mut pool, &data);
    assert!(pool[0].age_s >= keeps, "the bread in the Barn is past its time");
    data.get::<Mutex<Vec<TransferOp>>>("inventory_transfer_ops").unwrap().lock().unwrap().push(TransferOp {
        item_id: "bread_0".into(),
        qty: 1,
        add: true,
        age_s: pool[0].age_s,
        ..Default::default()
    });
    crate::systems::inventory::InventorySystem::new().tick(&mut world, 0.0, &data);
    *data.get::<Mutex<Option<String>>>("consume_request").unwrap().lock().unwrap() = Some("bread_0".into());
    sys.tick(&mut world, 0.0, &data);
    assert!(world.get::<&StatusEffects>(p).unwrap().has("food_poisoning"), "the Barn's spoiled bread poisons");
}

/// Old stacks and old saves: a stack, a stored item and a vessel saved before the clock
/// existed (no age in the file) load fresh, at age 0.
#[test]
fn a_stack_saved_before_the_clock_loads_fresh() {
    let stack: ItemStack = serde_json::from_str(r#"{"item_id":"bread_0","quantity":2,"max_stack":99}"#).unwrap();
    assert_eq!(stack.age_s, 0.0);
    let placed: PlacedItem =
        serde_json::from_str(r#"{"key":"bread_0","name":"Bread","qty":2,"container":"1/1"}"#).unwrap();
    assert_eq!(placed.age_s, 0.0);
    let mut c = serde_json::to_value(Container::new("pantry_cabinet_bin", 700.0)).unwrap();
    c.as_object_mut().unwrap().remove("content_age_s");
    let c: Container = serde_json::from_value(c).unwrap();
    assert_eq!(c.content_age_s, 0.0);
}

/// FOOD KEEPS ITS AGE THROUGH A SAVE (first-hour audit S6): a stack in the backpack, an item
/// in the Barn and a vessel's contents come back from the file as old as they were, so a
/// restart is not a way to make food fresh again. The spoilage clock used to live only in
/// the running FoodSystem.
///
/// Seen red (see the module doc): "the backpack's bread is as old as it was / left: 0.0 /
/// right: 7200.0".
#[test]
fn food_keeps_its_age_through_a_save() {
    use crate::ecs::components::MachineInstanceId;
    let mut world = hecs::World::new();
    let mut inv = Inventory::new(8);
    inv.add_item_aged("bread_0", 2, 99, 0, 7200.0);
    world.spawn((
        crate::ecs::components::Controllable,
        inv,
        crate::systems::skills::PlayerSkills::new(),
        crate::ecs::components::Name("Astra".to_string()),
        crate::ecs::components::Appearance::default(),
        crate::ecs::components::Outfit::default(),
    ));
    let mut pantry = Container::new("pantry_cabinet_bin", 700.0);
    pantry.current_content_item = Some("bread_0".into());
    pantry.current_qty = 4;
    pantry.content_age_s = 900.0;
    world.spawn((MachineInstanceId("pantry_1".into()), pantry));
    let mut save = crate::save_load::extract_world_save(&world);
    save.placed_items = Some(vec![PlacedItem { age_s: 40_000.0, ..barn("flour_0", 5) }]);
    let save: crate::persistence::WorldSave = serde_json::from_str(&serde_json::to_string(&save).unwrap()).unwrap();

    let mut fresh = hecs::World::new();
    let p = fresh.spawn((
        crate::ecs::components::Controllable,
        Inventory::new(8),
        crate::systems::skills::PlayerSkills::new(),
        crate::ecs::components::Name("X".to_string()),
        crate::ecs::components::Appearance::default(),
        crate::ecs::components::Outfit::default(),
    ));
    let vessel = fresh.spawn((MachineInstanceId("pantry_1".into()), Container::new("pantry_cabinet_bin", 700.0)));
    crate::save_load::apply_save_to_world(&mut fresh, &save);
    let mut gui = crate::gui::GuiState::default();
    crate::save_load::after_resume(&mut gui, &save, &crate::save_load::Resumed::default());
    assert_eq!(age_of(&fresh, p, "bread_0"), 7200.0, "the backpack's bread is as old as it was");
    assert_eq!(fresh.get::<&Container>(vessel).unwrap().content_age_s, 900.0, "the pantry's bread too");
    assert_eq!(gui.placed_items[0].age_s, 40_000.0, "and the Barn's flour");
}

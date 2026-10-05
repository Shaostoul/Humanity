//! The home's automated machines take their inputs from home storage, the
//! store they are fed from, and never from the player's backpack (BUG-150,
//! 2026-10-04). In real life a machine takes what is put into it or what is
//! in the store it is fed from, never what is in your pockets: the home's
//! sawmill used to saw the logs the Crafting page's "Dev: stock all
//! materials" had just put in the backpack, before the player could craft
//! the raft that needs them. Each test was seen red first; its doc comment
//! quotes the failure.

use std::collections::HashMap;
use std::sync::Mutex;

use super::{CraftingSystem, RecipeRegistry};
use crate::ecs::components::{AutoRefine, Controllable, MachineInstanceId};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::inventory::{Inventory, ItemRegistry};

/// The real recipes and items, home storage holding `stock`, the channel the
/// main loop files machine output from (as in a session), and an empty
/// 36-slot backpack under the player's control.
fn home(stock: &[(&str, u32)]) -> (DataStore, hecs::World, hecs::Entity) {
    let mut data = DataStore::new();
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/data/");
    let read = |f: &str| std::fs::read(format!("{root}{f}")).unwrap();
    data.insert("recipe_registry", RecipeRegistry::from_csv(&read("recipes.csv")).unwrap());
    data.insert("item_registry", ItemRegistry::from_csv(&read("items.csv")).unwrap());
    data.insert("player_notices", Mutex::new(Vec::<String>::new()));
    data.insert("auto_craft_status", Mutex::new(Vec::<String>::new()));
    let stock: HashMap<String, u32> = stock.iter().map(|(id, q)| (id.to_string(), *q)).collect();
    data.insert("home_stock", Mutex::new(stock));
    data.insert("home_stock_outputs", Mutex::new(Vec::<(String, u32)>::new()));
    super::register(&mut data);
    let mut world = hecs::World::new();
    let player = world.spawn((Inventory::new(36), Controllable));
    (data, world, player)
}

/// The home's sawmill, as world entry spawns it (home.ron: `saw_planks`).
fn sawmill(world: &mut hecs::World) {
    world.spawn((
        AutoRefine { recipe_id: "saw_planks".to_string(), keep: None },
        MachineInstanceId("sawmill_1".to_string()),
    ));
}

fn carried(world: &hecs::World, player: hecs::Entity, id: &str) -> u32 {
    world.get::<&Inventory>(player).unwrap().count_item(id)
}

fn stored(data: &DataStore, id: &str) -> u32 {
    data.get::<Mutex<HashMap<String, u32>>>("home_stock").unwrap().lock().unwrap().get(id).copied().unwrap_or(0)
}

/// Stand in for the main loop after a tick: what the machines filed joins
/// home storage (engine::stock_piles::receive_machine_outputs, then the next
/// frame's publish_home_stock).
fn settle(data: &DataStore) {
    let made: Vec<(String, u32)> =
        std::mem::take(&mut *data.get::<Mutex<Vec<(String, u32)>>>("home_stock_outputs").unwrap().lock().unwrap());
    let mut stock = data.get::<Mutex<HashMap<String, u32>>>("home_stock").unwrap().lock().unwrap();
    for (id, q) in made {
        *stock.entry(id).or_insert(0) += q;
    }
}

fn statuses(data: &DataStore) -> Vec<String> {
    data.get::<Mutex<Vec<String>>>("auto_craft_status").unwrap().lock().unwrap().clone()
}

/// The sawmill saws the 4 logs in home storage, two batches of 2 into 8
/// planks filed back in home storage, and leaves the 10 logs in the backpack
/// alone, however long it runs.
///
/// Seen red before the fix: "the backpack's logs are left alone" (left: 0,
/// right: 10): the sawmill took the backpack's logs first and sawed all ten.
#[test]
fn the_sawmill_saws_the_stored_logs_and_leaves_the_carried_ones_alone() {
    let (data, mut world, player) = home(&[("wood_log_0", 4)]);
    world.get::<&mut Inventory>(player).unwrap().add_item("wood_log_0", 10, 99);
    sawmill(&mut world);
    let mut sys = CraftingSystem::new();
    for _ in 0..40 {
        sys.tick(&mut world, 1.0, &data);
        settle(&data);
    }
    assert_eq!(carried(&world, player, "wood_log_0"), 10, "the backpack's logs are left alone");
    assert_eq!(stored(&data, "wood_log_0"), 0, "the stored logs were sawn");
    assert_eq!(stored(&data, "wood_plank_0"), 8, "two batches of planks, filed in home storage");
    assert_eq!(carried(&world, player, "wood_plank_0"), 0, "and none in the backpack");
}

/// With logs only in the backpack the sawmill does not start: it waits, and
/// its status line says what it is waiting for and where it looks, so a
/// backpack full of logs beside an idle sawmill is never a mystery.
///
/// Seen red before the fix: "the backpack's logs are left alone" (left: 0,
/// right: 10).
#[test]
fn a_machine_with_its_inputs_only_in_the_backpack_waits_for_home_storage() {
    let (data, mut world, player) = home(&[]);
    world.get::<&mut Inventory>(player).unwrap().add_item("wood_log_0", 10, 99);
    sawmill(&mut world);
    let mut sys = CraftingSystem::new();
    for _ in 0..30 {
        sys.tick(&mut world, 1.0, &data);
        settle(&data);
    }
    assert_eq!(carried(&world, player, "wood_log_0"), 10, "the backpack's logs are left alone");
    assert_eq!(stored(&data, "wood_plank_0"), 0, "nothing sawn");
    assert_eq!(statuses(&data), vec!["Saw Planks — waiting for Wood Log x2 in home storage".to_string()]);
}

/// Through the time away too (offline progression, crafting::away): the
/// machines run on home storage and leave the backpack alone.
///
/// Seen red before the fix: "the backpack's logs are left alone" (left: 0,
/// right: 10).
#[test]
fn through_the_time_away_the_machines_leave_the_backpack_alone() {
    let (data, mut world, player) = home(&[("wood_log_0", 4)]);
    world.get::<&mut Inventory>(player).unwrap().add_item("wood_log_0", 10, 99);
    sawmill(&mut world);
    super::away::hand_over(&data, Some(super::away::AwayWork { secs: 3600.0, ..Default::default() }));
    let mut sys = CraftingSystem::new();
    sys.tick(&mut world, 0.0, &data);
    assert_eq!(carried(&world, player, "wood_log_0"), 10, "the backpack's logs are left alone");
    let filed: u32 = data
        .get::<Mutex<Vec<(String, u32)>>>("home_stock_outputs")
        .unwrap()
        .lock()
        .unwrap()
        .iter()
        .filter(|(id, _)| id == "wood_plank_0")
        .map(|(_, q)| *q)
        .sum();
    assert_eq!(filed, 8, "two batches from the 4 stored logs, filed in home storage");
    assert_eq!(stored(&data, "wood_log_0"), 0, "the stored logs were sawn");
}

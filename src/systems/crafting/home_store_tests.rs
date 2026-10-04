//! Hand crafts that draw on the home's storage (BUG-147, crafting::home_store).
//! Each test was seen red first; its doc comment quotes the failure.

use std::collections::HashMap;
use std::sync::Mutex;

use super::home_store::{HAND_MADE_TO_STORAGE, HOME_STORAGE_HERE};
use super::{CraftingSystem, RecipeRegistry};
use crate::ecs::components::{Controllable, Vehicle};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::construction::StationsWhere;
use crate::systems::inventory::{Inventory, ItemRegistry};

/// The real recipes, items and grades, an empty backpack (36 slots, 65 L)
/// under the player's control, and home storage holding `stock`.
fn real_home(stock: &[(&str, u32)]) -> (DataStore, hecs::World, hecs::Entity) {
    let mut data = DataStore::new();
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/data/");
    let read = |f: &str| std::fs::read(format!("{root}{f}")).unwrap();
    data.insert("recipe_registry", RecipeRegistry::from_csv(&read("recipes.csv")).unwrap());
    data.insert("item_registry", ItemRegistry::from_csv(&read("items.csv")).unwrap());
    data.insert("quality_levels", super::quality::QualityLevels::from_ron(&read("manufacturing.ron")).unwrap());
    data.insert(
        "vehicle_kit_registry",
        crate::systems::vehicles::VehicleKitRegistry::from_ron(&read("vehicles/kits.ron")).unwrap(),
    );
    data.insert("dev_stock_materials", Mutex::new(false));
    data.insert("craft_request", Mutex::new(Option::<String>::None));
    data.insert("player_notices", Mutex::new(Vec::<String>::new()));
    let stock: HashMap<String, u32> = stock.iter().map(|(id, q)| (id.to_string(), *q)).collect();
    data.insert("home_stock", Mutex::new(stock));
    data.insert("home_stock_outputs", Mutex::new(Vec::<(String, u32)>::new()));
    super::register(&mut data);
    let mut world = hecs::World::new();
    let mut pack = Inventory::new(36);
    pack.ensure_slots(400);
    let player = world.spawn((pack, Controllable));
    (data, world, player)
}

fn parts(data: &DataStore, recipe: &str) -> Vec<(String, u32)> {
    data.get::<RecipeRegistry>("recipe_registry").unwrap().recipes[recipe].inputs.clone()
}

fn name_of(data: &DataStore, id: &str) -> String {
    data.get::<ItemRegistry>("item_registry").unwrap().items[id].name.clone()
}

fn craft(sys: &mut CraftingSystem, world: &mut hecs::World, data: &DataStore, recipe: &str) {
    *data.get::<Mutex<Option<String>>>("craft_request").unwrap().lock().unwrap() = Some(recipe.into());
    sys.tick(world, 0.016, data);
}

fn carried(world: &hecs::World, player: hecs::Entity, id: &str) -> u32 {
    world.get::<&Inventory>(player).unwrap().count_item(id)
}

fn stored(data: &DataStore, id: &str) -> u32 {
    data.get::<Mutex<HashMap<String, u32>>>("home_stock").unwrap().lock().unwrap().get(id).copied().unwrap_or(0)
}

fn put_in_storage(data: &DataStore, id: &str, qty: u32) {
    data.get::<Mutex<HashMap<String, u32>>>("home_stock").unwrap().lock().unwrap().insert(id.into(), qty);
}

fn hand_made(data: &DataStore) -> Vec<(String, u32, u8)> {
    data.get::<Mutex<Vec<(String, u32, u8)>>>(HAND_MADE_TO_STORAGE).unwrap().lock().unwrap().clone()
}

fn notices(data: &DataStore) -> Vec<String> {
    data.get::<Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap().clone()
}

fn vehicles(world: &hecs::World) -> Vec<String> {
    world.query::<&Vehicle>().iter().map(|(_e, v)| v.item_id.clone()).collect()
}

/// A spacecraft pod (about 2,200 L of parts in 158 stacks) is built from
/// half its parts in the backpack and the rest in home storage: the backpack
/// is spent first (its spare titanium stays), storage gives exactly what the
/// backpack lacked (its 3 spare of each stay), and the finished pod, far too
/// big for the backpack, is filed in home storage with the crafter's grade.
/// Seen red before the fix: "the craft started: []" (left: 0, right: 1):
/// refused, because the backpack alone was short. And against a split that
/// took storage first: "titanium_ingot_0: the backpack is used first"
/// (left: 7, right: 2).
#[test]
fn a_vehicle_built_from_the_backpack_and_home_storage_uses_exactly_its_parts() {
    let (data, mut world, player) = real_home(&[]);
    let bill = parts(&data, "build_spacecraft_pod");
    assert!(bill.len() > 20, "the pod's real bill of materials: {bill:?}");
    let split = |i: usize, need: u32| if i == 0 { (need + 2, 5) } else { (need / 2, need - need / 2 + 3) };
    for (i, (id, need)) in bill.iter().enumerate() {
        let (pack, home) = split(i, *need);
        world.get::<&mut Inventory>(player).unwrap().add_item(id, pack, 999);
        put_in_storage(&data, id, home);
    }
    let mut sys = CraftingSystem::new();
    craft(&mut sys, &mut world, &data, "build_spacecraft_pod");
    assert_eq!(sys.active_crafts.len(), 1, "the craft started: {:?}", notices(&data));
    for (i, (id, _)) in bill.iter().enumerate() {
        let (pack_left, home_left) = if i == 0 { (2, 5) } else { (0, 3) };
        assert_eq!(carried(&world, player, id), pack_left, "{id}: the backpack is used first");
        assert_eq!(stored(&data, id), home_left, "{id}: home storage gave exactly what the backpack lacked");
    }
    for _ in 0..10 {
        sys.tick(&mut world, 100.0, &data); // a 900 s craft
    }
    assert!(sys.active_crafts.is_empty(), "finished, not waiting for room: {:?}", notices(&data));
    assert_eq!(carried(&world, player, "spacecraft_pod_0"), 0, "a pod does not go in a backpack");
    let made = hand_made(&data);
    assert_eq!(made.len(), 1, "{made:?}");
    let (id, qty, grade) = &made[0];
    assert_eq!((id.as_str(), *qty), ("spacecraft_pod_0", 1));
    assert!(*grade > 0, "a hand-made pod keeps its grade in storage");
    assert!(notices(&data).iter().any(|n| n.contains("home storage")), "{:?}", notices(&data));
}

/// One glue pot short across the backpack and home storage together: the
/// craft is refused, nothing at all is spent from either, and the player is
/// told what is missing and that both places were counted. Seen red before
/// the fix: "the player is told what is short: []" (a short craft was
/// refused with only a log line).
#[test]
fn a_craft_short_in_both_places_is_refused_and_spends_nothing() {
    let (data, mut world, player) = real_home(&[]);
    let bill = parts(&data, "build_spacecraft_pod");
    let last = bill.len() - 1;
    for (i, (id, need)) in bill.iter().enumerate() {
        let (pack, home) = if i == last { (0, need - 1) } else { (need / 2, need - need / 2) };
        world.get::<&mut Inventory>(player).unwrap().add_item(id, pack, 999);
        put_in_storage(&data, id, home);
    }
    let mut sys = CraftingSystem::new();
    craft(&mut sys, &mut world, &data, "build_spacecraft_pod");
    let (short_id, _) = &bill[last];
    let short_name = name_of(&data, short_id);
    let told = notices(&data);
    assert!(
        told.iter().any(|n| n.contains(&format!("1 more {short_name}")) && n.contains("home storage")),
        "the player is told what is short: {told:?}"
    );
    assert!(sys.active_crafts.is_empty(), "refused");
    for (i, (id, need)) in bill.iter().enumerate() {
        let (pack, home) = if i == last { (0, need - 1) } else { (need / 2, need - need / 2) };
        assert_eq!(carried(&world, player, id), pack, "{id}: nothing spent from the backpack");
        assert_eq!(stored(&data, id), home, "{id}: nothing spent from home storage");
    }
}

/// Nothing but the player's own home ever feeds a hand craft. Another
/// player's things can only appear in this game as entities that are not the
/// player (here a neighbour's figure carrying every part in its own pack):
/// the relay holds no one's items and a neighbour's home is drawn empty. And
/// a guest on a shared ship, standing on someone else's plot, has no home
/// storage there at all: its own home is put away, so the stock this game
/// mirrors (which is the guest's own) does not count either. Refused, nothing
/// touched anywhere, and told why. Seen red before the fix, which touched
/// nothing but said nothing ("[]", no notice), and with `HomeStore::here`
/// ignoring HOME_STORAGE_HERE: "refused: []" (the craft started on the
/// put-away home's storage).
#[test]
fn another_players_home_storage_is_never_touched() {
    let (mut data, mut world, player) = real_home(&[]);
    data.insert(HOME_STORAGE_HERE, Mutex::new(false));
    data.insert("stations_where", Mutex::new(StationsWhere::Home));
    let bill = parts(&data, "build_spacecraft_pod");
    let mut neighbours_pack = Inventory::new(36);
    neighbours_pack.ensure_slots(400);
    for (id, need) in &bill {
        world.get::<&mut Inventory>(player).unwrap().add_item(id, need / 2, 999);
        put_in_storage(&data, id, need + 3);
        neighbours_pack.add_item(id, need * 2, 999);
    }
    let neighbour = world.spawn((neighbours_pack,));
    let mut sys = CraftingSystem::new();
    craft(&mut sys, &mut world, &data, "build_spacecraft_pod");
    assert!(sys.active_crafts.is_empty(), "refused: {:?}", notices(&data));
    for (id, need) in &bill {
        assert_eq!(world.get::<&Inventory>(neighbour).unwrap().count_item(id), need * 2, "{id}: the neighbour's untouched");
        assert_eq!(stored(&data, id), need + 3, "{id}: home storage untouched");
        assert_eq!(carried(&world, player, id), need / 2, "{id}: the backpack untouched");
    }
    assert!(notices(&data).iter().any(|n| n.contains("not on this ship")), "{:?}", notices(&data));
}

/// Where a craft's result goes: into the backpack when it fits (as before);
/// aboard the home, what the backpack cannot take goes to home storage with
/// its grade instead of the craft being refused; away from the home (open
/// space here) a craft whose result does not fit is still refused before
/// anything is spent. Seen red before the fix: "aboard, the cart went to
/// home storage: [] [\"No room in your backpack for Build Hand Cart: make
/// room and craft again.\"]" (left: 0, right: 1).
#[test]
fn a_result_too_big_for_the_backpack_goes_to_home_storage() {
    let (mut data, mut world, player) = real_home(&[]);
    // A hand-cart (7.6 L) from a backpack with room: into the backpack.
    for (id, need) in parts(&data, "build_hand_cart") {
        world.get::<&mut Inventory>(player).unwrap().add_item(&id, need * 3, 999);
    }
    let mut sys = CraftingSystem::new();
    craft(&mut sys, &mut world, &data, "build_hand_cart");
    sys.tick(&mut world, 100.0, &data);
    assert_eq!(carried(&world, player, "cart_hand_0"), 1, "it fits: the backpack");
    assert!(hand_made(&data).is_empty());

    // The backpack full: aboard, the second cart goes to home storage.
    world.get::<&mut Inventory>(player).unwrap().volume_current_l = 64.0;
    craft(&mut sys, &mut world, &data, "build_hand_cart");
    sys.tick(&mut world, 100.0, &data);
    let made = hand_made(&data);
    assert_eq!(made.len(), 1, "aboard, the cart went to home storage: {made:?} {:?}", notices(&data));
    assert_eq!((made[0].0.as_str(), made[0].1), ("cart_hand_0", 1));
    assert!(made[0].2 > 0, "with its grade");
    assert_eq!(carried(&world, player, "cart_hand_0"), 1, "not in the backpack");

    // In open space the home's storage is not there: refused, nothing spent.
    data.insert("stations_where", Mutex::new(StationsWhere::Nowhere));
    let before: Vec<u32> = parts(&data, "build_hand_cart").iter().map(|(id, _)| carried(&world, player, id)).collect();
    craft(&mut sys, &mut world, &data, "build_hand_cart");
    sys.tick(&mut world, 100.0, &data);
    let after: Vec<u32> = parts(&data, "build_hand_cart").iter().map(|(id, _)| carried(&world, player, id)).collect();
    assert_eq!(before, after, "nothing spent");
    assert_eq!(hand_made(&data).len(), 1, "nothing more filed");
    assert!(notices(&data).iter().any(|n| n.contains("No room in your backpack")), "{:?}", notices(&data));
}

/// A vehicle the game already rolls out as a world vehicle (the rover)
/// still does, when its parts came out of home storage: nothing is filed in
/// storage and nothing lands in the backpack. Seen red before the fix:
/// "rolled out: []" (left: [], right: ["rover_0"]): the backpack alone could
/// not cover the rover's parts.
#[test]
fn a_rover_from_home_storage_still_rolls_out() {
    let (data, mut world, player) = real_home(&[]);
    for (id, need) in parts(&data, "build_rover") {
        world.get::<&mut Inventory>(player).unwrap().add_item(&id, need / 3, 999);
        put_in_storage(&data, &id, need - need / 3);
    }
    let mut sys = CraftingSystem::new();
    craft(&mut sys, &mut world, &data, "build_rover");
    for _ in 0..6 {
        sys.tick(&mut world, 100.0, &data); // a 480 s craft
    }
    assert_eq!(vehicles(&world), vec!["rover_0".to_string()], "rolled out: {:?}", notices(&data));
    assert!(hand_made(&data).is_empty(), "a vehicle is not filed in storage");
    assert_eq!(carried(&world, player, "rover_0"), 0);
    for (id, _) in parts(&data, "build_rover") {
        assert_eq!((carried(&world, player, &id), stored(&data, &id)), (0, 0), "{id}: all of it used");
    }
}

//! From systems/food_spoilage_tests.rs: moving food keeps its age, moved back to storage by the GUI
//! (`gui::return_to_storage`).

use crate::ecs::systems::System;
use crate::systems::food::spoilage_tests::age_of;
use crate::systems::food::spoilage_tests::barn;
use crate::systems::food::spoilage_tests::player;
use crate::systems::food::spoilage_tests::store;
use crate::systems::food::spoilage_tests::vessel;
use crate::systems::inventory::Inventory;
use crate::systems::inventory::TransferOp;
use crate::systems::inventory::containers::Container;
use crate::systems::inventory::containers::ContainerRegistry;
use crate::systems::inventory::placed::PlacedItem;
use std::sync::Mutex;

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
    crate::gui::return_to_storage(&mut pool, "bread_0", 2, Some(&origin), "Bread");
    assert_eq!(pool[0].age_s, 3750.0, "what the backpack could not take goes back as old as it was");
}

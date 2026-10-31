//! From systems/food_spoilage_tests.rs: food keeps its age through a save, saved and loaded by the
//! engine (`save_load`).

use crate::systems::food::spoilage_tests::age_of;
use crate::systems::food::spoilage_tests::barn;
use crate::systems::inventory::Inventory;
use crate::systems::inventory::containers::Container;
use crate::systems::inventory::placed::PlacedItem;

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

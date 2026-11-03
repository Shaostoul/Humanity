//! From systems/farming/weed_tests.rs: an area's weeds through a save and load (`save_load`).

use humanity_core::ecs::components::{AreaWeeds, SoilMemory};
use humanity_systems::farming::fixtures::weeds::*;

/// The weeds are saved: they live on `SoilMemory`, which the world save
/// writes whole, so a round trip keeps an area's cover, seed bank, notice and
/// mulch days; and a save from before weeds existed loads with none. Seen red
/// by marking `SoilMemory::weeds` `#[serde(skip)]` (the bed came back empty).
/// Native only: the world save lives in the desktop build (save_load and
/// persistence are native-gated), as in soil_ph_tests.
#[cfg(feature = "native")]
#[test]
fn weeds_survive_a_save_and_old_saves_load_with_none() {
    use crate::save_load::{apply_save_to_world, extract_world_save};
    let mut world = hecs::World::new();
    let mut memory_in = SoilMemory::default();
    let mut w = AreaWeeds { level: 0.37, bank: 1.4, told: true, ..Default::default() };
    w.mulch.insert("mulch_bark".into(), 812.5);
    memory_in.weeds.insert("tomato_bed".into(), w.clone());
    world.spawn((memory_in,));
    let text = serde_json::to_string(&extract_world_save(&world)).unwrap();
    let back: humanity_systems::persistence::WorldSave = serde_json::from_str(&text).unwrap();
    let mut fresh = hecs::World::new();
    apply_save_to_world(&mut fresh, &back);
    assert_eq!(memory(&fresh).weeds.get("tomato_bed"), Some(&w), "the bed's weeds came back");
    let mut old: serde_json::Value = serde_json::from_str(&text).unwrap();
    old["soil_memory"].as_object_mut().unwrap().remove("weeds");
    let back: humanity_systems::persistence::WorldSave = serde_json::from_value(old).unwrap();
    let mut older = hecs::World::new();
    apply_save_to_world(&mut older, &back);
    assert!(memory(&older).weeds.is_empty(), "an old save loads with no weeds");
}

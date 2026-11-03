//! From systems/farming/soil_ph_tests.rs: a unit's pH through a save and load (`save_load`).

use humanity_systems::farming::soil_ph::{self, NITRIFICATION};
use humanity_systems::farming::fixtures::soil_ph::*;

/// A unit's pH and what is still reacting come back after a save; a save
/// from before pH existed loads with none, and its units read the bed's
/// starting 6.5. Seen red by marking `SoilMemory::ph` `#[serde(skip)]` (the
/// record came back empty).
#[cfg(feature = "native")]
#[test]
fn ph_survives_a_save_and_old_saves_read_the_default() {
    use humanity_systems::persistence::WorldSave;
    use crate::save_load::{apply_save_to_world, extract_world_save};
    let data = store(true);
    let ph = shipped();
    let mut world = hecs::World::new();
    world.spawn((crop(&data, "lettuce", "bed_1", 3),));
    set_unit(&mut world, "bed_1", 3, 5.7, &[("lime", 0.4), (NITRIFICATION, -0.05)]);
    let before = unit(&world, "bed_1", 3).unwrap();
    let text = serde_json::to_string(&extract_world_save(&world)).unwrap();
    let back: WorldSave = serde_json::from_str(&text).unwrap();
    let mut fresh = hecs::World::new();
    apply_save_to_world(&mut fresh, &back);
    assert_eq!(unit(&fresh, "bed_1", 3), Some(before), "the unit's pH round-trips");

    let mut old: serde_json::Value = serde_json::from_str(&text).unwrap();
    old["soil_memory"].as_object_mut().unwrap().remove("ph");
    let back: WorldSave = serde_json::from_value(old).unwrap();
    let mut fresh = hecs::World::new();
    apply_save_to_world(&mut fresh, &back);
    assert!(memory(&fresh).ph.is_empty(), "an old save has no pH");
    assert_eq!(soil_ph::crop_ph(&memory(&fresh).ph, &ph, "bed_1", Some(3)), Some((6.5, false)));
}

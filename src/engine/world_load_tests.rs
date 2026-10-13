//! What a home's start carries of the homestead's world (ship homes increment 6, lane 2A): the
//! herd and the wild spawns are spawned only for a start that lists them in data/homes/starts.ron
//! (`home_content`), and taken away for one that does not.

use super::*;
use crate::hot_reload::data_store::DataStore;
use crate::ship::starts::Starts;
use std::collections::HashMap;

fn repo_data() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// The registries the herd and the wilds are spawned from, as the world load has them
/// (engine/registries.rs): the creatures, the items, the starter herd's and the wilds' lists.
fn registries() -> DataStore {
    let root = repo_data();
    let read = |f: &str| std::fs::read(root.join(f)).unwrap_or_else(|e| panic!("{f}: {e}"));
    let mut data = DataStore::new();
    data.insert("creature_registry", crate::systems::livestock::CreatureRegistry::from_csv(&read("creatures.csv")).unwrap());
    data.insert("item_registry", crate::systems::inventory::ItemRegistry::from_csv(&read("items.csv")).unwrap());
    data.insert("livestock_spawn_list", crate::systems::livestock::LivestockSpawnList::from_ron(&read("entities/livestock.ron")).unwrap());
    data.insert("wild_spawn_list", crate::systems::livestock::WildSpawnList::from_ron(&read("entities/wild_spawns.ron")).unwrap());
    crate::systems::livestock::register(&mut data);
    data
}

/// A CABIN HOME SPAWNS NONE OF THE HOMESTEAD'S WORLD (plan 3.7). The homestead's start carries its
/// herd, the wild spawns, the decoration plants, the showcase garden and the hologram
/// (`home_content` in data/homes/starts.ron); the Cabin's carries none of them: a cabin has no
/// fields to keep a herd near, and the wilds stand at fixed points of the homestead's acre. So the
/// world of a homestead save has its herd and its wilds, and the same spawns for a cabin save (a
/// swap into one, `refresh_home_content`, or a world load of one) leave no creature at all.
///
/// Seen red 2026-10-12 with `spawn_herd` ignoring `carries` (the herd spawned for every home, as
/// before increment 6): "a cabin home has the homestead's world: herd 7, wilds 0, creatures 7".
#[test]
fn a_cabin_home_spawns_none_of_the_homesteads_world() {
    let starts = Starts::load(&repo_data());
    let content = |start: &str| starts.get(start).unwrap_or_else(|| panic!("the {start} start")).home_content.clone();
    let (homestead, cabin) = (content("homestead"), content("cabin"));
    for c in [HomeContent::Herd, HomeContent::WildSpawns, HomeContent::Decorations, HomeContent::ShowcaseGarden, HomeContent::Hologram] {
        assert!(homestead.contains(&c), "the homestead carries {c:?}");
        assert!(!cabin.contains(&c), "a cabin carries {c:?}");
    }
    let data = registries();
    // Every machine the herd is placed near, standing in the home.
    let anchors: HashMap<String, Vec3> = data
        .get::<crate::systems::livestock::LivestockSpawnList>("livestock_spawn_list")
        .expect("the herd's list")
        .animals
        .iter()
        .map(|a| (a.near.clone(), Vec3::new(20.0, 0.0, 30.0)))
        .collect();
    let mut world = hecs::World::new();
    let herd = spawn_herd(&mut world, &data, &anchors, homestead.contains(&HomeContent::Herd));
    let wilds = spawn_wilds(&mut world, &data, false, homestead.contains(&HomeContent::WildSpawns));
    assert!(herd > 0, "a homestead home has its herd");
    assert!(wilds > 0, "a homestead home has its wilds (the berry bushes, the wild flax, the clay pit)");
    // A cabin's: the same spawns take every one of them away and put none back.
    let herd = spawn_herd(&mut world, &data, &anchors, cabin.contains(&HomeContent::Herd));
    let wilds = spawn_wilds(&mut world, &data, false, cabin.contains(&HomeContent::WildSpawns));
    let creatures = world.query::<&crate::ecs::components::Creature>().iter().count();
    assert_eq!(
        (herd, wilds, creatures),
        (0, 0, 0),
        "a cabin home has the homestead's world: herd {herd}, wilds {wilds}, creatures {creatures}"
    );
}

/// A home on the ship's ventilation has no sealed air of its own: the old space goes and none
/// comes (`replace_home_air`), while a homestead's is made again at its own size.
#[test]
fn a_home_on_the_ships_air_has_no_air_space_of_its_own() {
    use crate::systems::atmosphere::{EnclosedSpace, HomeAir};
    let mut world = hecs::World::new();
    replace_home_air(&mut world, true, 2_000.0, 14_685.0);
    let volumes: Vec<f32> = world.query::<(&HomeAir, &EnclosedSpace)>().iter().map(|(_, (_, s))| s.volume_m3).collect();
    assert_eq!(volumes, vec![14_685.0], "a homestead has its own air, at its own size");
    replace_home_air(&mut world, false, 0.0, 19.5);
    assert_eq!(world.query::<&HomeAir>().iter().count(), 0, "a cabin keeps the homestead's air space");
}

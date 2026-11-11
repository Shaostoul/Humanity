//! What a home's start carries of the homestead's world (ship homes increment 6, lane 2A): the
//! herd and the wild spawns are spawned only for a start that lists them in data/homes/starts.ron
//! (`home_content`), and taken away for one that does not.

use super::*;
use humanity_core::hot_reload::data_store::DataStore;
use humanity_ship::starts::Starts;
use std::collections::HashMap;

fn repo_data() -> std::path::PathBuf {
    std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data")
}

/// The registries the herd and the wilds are spawned from, as the world load has them
/// (engine/registries.rs): the creatures, the items, the starter herd's and the wilds' lists.
fn registries() -> DataStore {
    let root = repo_data();
    let read = |f: &str| std::fs::read(root.join(f)).unwrap_or_else(|e| panic!("{f}: {e}"));
    let mut data = DataStore::new();
    data.insert("creature_registry", humanity_systems::livestock::CreatureRegistry::from_csv(&read("creatures.csv")).unwrap());
    data.insert("item_registry", humanity_systems::inventory::ItemRegistry::from_csv(&read("items.csv")).unwrap());
    data.insert("livestock_spawn_list", humanity_systems::livestock::LivestockSpawnList::from_ron(&read("entities/livestock.ron")).unwrap());
    data.insert("wild_spawn_list", humanity_systems::livestock::WildSpawnList::from_ron(&read("entities/wild_spawns.ron")).unwrap());
    humanity_systems::livestock::register(&mut data);
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
        .get::<humanity_systems::livestock::LivestockSpawnList>("livestock_spawn_list")
        .expect("the herd's list")
        .animals
        .iter()
        .map(|a| (a.near.clone(), Vec3::new(20.0, 0.0, 30.0)))
        .collect();
    let mut world = hecs::World::new();
    let herd = spawn_herd(&mut world, &data, &anchors, homestead.contains(&HomeContent::Herd));
    let wilds = spawn_wilds(&mut world, &data, 0.0, homestead.contains(&HomeContent::WildSpawns));
    assert!(herd > 0, "a homestead home has its herd");
    assert!(wilds > 0, "a homestead home has its wilds (the berry bushes, the wild flax, the clay pit)");
    // A cabin's: the same spawns take every one of them away and put none back.
    let herd = spawn_herd(&mut world, &data, &anchors, cabin.contains(&HomeContent::Herd));
    let wilds = spawn_wilds(&mut world, &data, 0.0, cabin.contains(&HomeContent::WildSpawns));
    let creatures = world.query::<&humanity_core::ecs::components::Creature>().iter().count();
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
    use humanity_systems::atmosphere::{EnclosedSpace, HomeAir};
    let mut world = hecs::World::new();
    replace_home_air(&mut world, true, 2_000.0, 14_685.0);
    let volumes: Vec<f32> = world.query::<(&HomeAir, &EnclosedSpace)>().iter().map(|(_, (_, s))| s.volume_m3).collect();
    assert_eq!(volumes, vec![14_685.0], "a homestead has its own air, at its own size");
    replace_home_air(&mut world, false, 0.0, 19.5);
    assert_eq!(world.query::<&HomeAir>().iter().count(), 0, "a cabin keeps the homestead's air space");
}

/// HOSTILE WILDLIFE'S LEVELS (2026-11-11, data/gameplay/levels.json): Off spawns no predator at
/// all, Few fewer than Normal (one of each pack at least), Normal every pack whole; the peaceful
/// wilds (berry bushes, flax, clay) are the same at every level.
///
/// Seen red 2026-11-11 with the share ignored (any share above 0 spawning whole packs, the old
/// switch's On): "Few spawns fewer predators than Normal (3 against 3)".
#[test]
fn hostile_wildlife_off_few_and_normal() {
    // The shipped wilds hold no pack today (the wolves wait for the predator arc), so the test
    // brings back the file's own wolf row as a pack of three.
    let mut data = registries();
    let text = std::fs::read_to_string(repo_data().join("entities/wild_spawns.ron")).unwrap();
    let with_wolves = text.replacen(r#"// (creature: "wolf", count: 2,"#, r#"(creature: "wolf", count: 3,"#, 1);
    assert_ne!(with_wolves, text, "the wolf row is in the file");
    data.insert("wild_spawn_list", humanity_systems::livestock::WildSpawnList::from_ron(with_wolves.as_bytes()).unwrap());
    let count = |share: f32| {
        let mut world = hecs::World::new();
        let all = spawn_wilds(&mut world, &data, share, true);
        let hostile = world
            .query::<&humanity_core::ecs::components::AIBehavior>()
            .iter()
            .filter(|(_, ai)| matches!(ai.behavior_type.as_str(), "predator" | "aggressive"))
            .count();
        (all - hostile, hostile)
    };
    let (calm_off, off) = count(0.0);
    let (calm_few, few) = count(0.34);
    let (calm_normal, normal) = count(1.0);
    assert_eq!(off, 0, "Off spawns no predator");
    assert!(normal > 0, "Normal spawns the packs ({normal})");
    assert!(few > 0 && few < normal, "Few spawns fewer predators than Normal ({few} against {normal})");
    assert_eq!((calm_off, calm_few), (calm_normal, calm_normal), "the peaceful wilds are the same at every level");
}

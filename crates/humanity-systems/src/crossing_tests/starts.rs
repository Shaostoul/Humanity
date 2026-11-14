//! From ship/starts.rs's tests: the starts are data, every kit item a real item of the
//! simulation's and every opening a real quest.

use humanity_ship::starts::*;
use std::path::{Path, PathBuf};

fn data_dir() -> PathBuf {
    Path::new(env!("HUMANITY_REPO_ROOT")).join("data")
}

/// The repo's starts file as the game reads it from disk, held to the rules (as in
/// ship/starts.rs's tests).
fn the_file() -> Starts {
    Starts::parse(&std::fs::read(data_dir().join(STARTS_FILE)).expect("data/homes/starts.ron is in the repo"))
        .unwrap_or_else(|e| panic!("data/homes/starts.ron: {e}"))
}

/// The starts are data, every offered one can be built and lived in, and none is called a
/// tier: three rows with unique ids, the default offered, every offered row's design built
/// into the exe with the kind of its plots, every household file loading with every machine
/// it places defined and every one of its own standing inside the design, every kit item a
/// real item, every opening a real quest, every storage seed a places file with the backpack
/// in it, and no word a player reads saying "tier", "rank" or "level" (a start is how much
/// space you hold, never a step on a ladder: engagement-modes.md).
///
/// Seen red 2026-10-12 with the apartment flipped to `offered: true`: "the apartment start is
/// offered but no apartment design is built in" failed.
#[test]
fn the_starts_are_data_and_none_is_called_a_tier() {
    let starts = the_file();
    // Cabin, Apartment and Homestead for players, and since 2026-11-13 the room program's two
    // homesteads as drafts for developers (`dev`, offered in the Dev play mode only), and the same
    // day the four row houses of Row Street.
    assert_eq!(starts.starts.iter().filter(|s| !s.dev).count(), 3, "Cabin, Apartment and Homestead");
    assert_eq!(starts.starts.iter().filter(|s| s.dev).map(|s| s.id.as_str()).collect::<Vec<_>>(), ["household_homestead", "one_person_homestead", "row_house_two_bedroom", "row_house_three_bedroom", "row_house_couples", "row_house_friends"]);
    let ids: std::collections::BTreeSet<&str> = starts.starts.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids.len(), 9, "the ids are unique: {ids:?}");
    assert!(starts.default_start().offered, "the default start is offered");
    assert_eq!(starts.default_start().id, starts.default);

    let items = crate::inventory::ItemRegistry::from_csv(&std::fs::read(data_dir().join("items.csv")).unwrap())
        .expect("items.csv parses");
    let quests = crate::quests::QuestRegistry::from_ron_dir(&data_dir().join("quests"));
    assert!(quests.get("gs_first_steps").is_some(), "the quests parsed");
    // Every start a game may make, the drafts for developers too.
    for s in starts.starts.iter().filter(|s| s.offered) {
        let design = humanity_ship::ship_structure::HomeDesign::built_in(&s.design)
            .unwrap_or_else(|| panic!("the {} start is offered but no {} design is built in", s.id, s.design));
        assert_eq!(design.kind, s.plot_kind, "the {} start's design is for {} plots, not {}", s.id, design.kind, s.plot_kind);
        let (w, d) = (design.body.width, design.body.depth);
        for h in &s.households {
            let path = data_dir().join("machines").join(&h.machines);
            let home = crate::grow_machines::load_home(&path)
                .unwrap_or_else(|| panic!("the {} start's {} household file {} does not load", s.id, h.id, path.display()));
            for i in &home.instances {
                assert!(home.catalog.contains_key(&i.machine), "{}: {} is a {} the catalog does not define", h.machines, i.id, i.machine);
                if i.zone == "home" {
                    let (x, z) = (i.offset.0, i.offset.2);
                    assert!(
                        (0.0..=w).contains(&x) && (0.0..=d).contains(&z),
                        "{}: {} at ({x}, {z}) stands outside the {} design's {w} x {d} m",
                        h.machines,
                        i.id,
                        s.design
                    );
                }
            }
            for a in &home.arrays {
                assert!(home.catalog.contains_key(&a.machine), "{}: an array of {} the catalog does not define", h.machines, a.machine);
            }
        }
        for (item, n) in &s.kit {
            assert!(items.items.contains_key(item), "the {} start's kit carries {item}, which is not in items.csv", s.id);
            assert!(*n > 0);
        }
        if let Some(q) = &s.opening {
            assert!(quests.get(q).is_some(), "the {} start opens with {q}, which is not a quest", s.id);
        }
        if !s.storage.is_empty() {
            let text = std::fs::read_to_string(data_dir().join(&s.storage))
                .unwrap_or_else(|e| panic!("the {} start's storage {}: {e}", s.id, s.storage));
            let seed: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", s.storage));
            let entities = seed["entities"].as_array().unwrap_or_else(|| panic!("{} lists no entities", s.storage));
            let backpack = entities.iter().flat_map(|e| e["children"].as_array().into_iter().flatten()).any(|c| c["kind"] == "backpack");
            assert!(backpack, "{} has no backpack node for the live backpack", s.storage);
        }
    }
    for s in &starts.starts {
        let mut words = vec![s.name.clone(), s.sentence.clone(), s.not_offered_why.clone()];
        words.extend(s.households.iter().map(|h| h.name.clone()));
        for text in &words {
            for word in text.to_lowercase().split(|c: char| !c.is_ascii_alphabetic()) {
                assert!(!["tier", "tiers", "rank", "ranks", "level", "levels"].contains(&word), "the {} start says {word:?}: {text:?}", s.id);
            }
        }
    }
}

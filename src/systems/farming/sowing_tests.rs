//! Sowing from the backpack and the crop outlook (2026-10-12, first-hour
//! audit F11; sowing.rs, outlook.rs).
//!
//! Each test names the deliberate break it was seen failing against.

use std::collections::HashMap;
use std::sync::Mutex;

use glam::Vec3;

use super::gardening_tests::make_store;
use super::lighting::GrowPlot;
use super::outlook::{self, CropOutlook, WaterSupply};
use super::sowing;
use super::*;
use crate::ecs::components::{Controllable, CropInstance, Irrigator, STAGE_DEAD};
use crate::hot_reload::data_store::DataStore;
use crate::systems::grow_machines::Takes;
use crate::systems::inventory::Inventory;

/// A small home as the engine publishes it: two towers of one design 20 m
/// apart (the first owns the design id), a two-plot potato bed, a five-shelf
/// mushroom rack 4 m from the bed, and a fish tank 2 m from it.
fn home() -> DataStore {
    let mut data = make_store();
    data.insert("player_notices", Mutex::new(Vec::<String>::new()));
    let tower = |id: &str, z: f32| GrowPlot {
        id: id.into(),
        aliases: vec!["nutrition".into()],
        pos: [0.0, 0.0, z],
        footprint_m2: 0.09,
        cups: 4,
        ..Default::default()
    };
    let machine = |id: &str, x: f32, z: f32, m2: f32| GrowPlot { id: id.into(), pos: [x, 0.0, z], footprint_m2: m2, ..Default::default() };
    data.insert(
        "grow_plots",
        vec![
            tower("ntower_0", 0.0),
            tower("ntower_1", 20.0),
            machine("potato_grow_bed_1", 10.0, 0.0, 2.0),
            machine("mushroom_rack_1", 10.0, 4.0, 0.72),
            machine("aquaponic_tank_1", 8.0, 0.0, 2.56),
        ],
    );
    let mut inst: HashMap<String, Vec<(String, u32)>> = HashMap::new();
    inst.insert("potato_grow_bed".into(), vec![("potato_grow_bed_1".into(), 2)]);
    inst.insert("mushroom_rack".into(), vec![("mushroom_rack_1".into(), 5)]);
    inst.insert("aquaponic_tank".into(), vec![("aquaponic_tank_1".into(), 1)]);
    data.insert("grow_instances", inst);
    data
}

/// Stand at (x, z) on the deck at height 0: the camera is the eye, 1.7 m up.
fn stand(data: &mut DataStore, x: f32, z: f32) {
    data.insert("camera_position", Vec3::new(x, crate::ecs::components::EYE_HEIGHT_M as f32, z));
}

fn player(world: &mut hecs::World, seeds: &[(&str, u32)]) -> hecs::Entity {
    let mut inv = Inventory::new(16);
    for (id, n) in seeds {
        inv.add_item(id, *n, 99);
    }
    world.spawn((inv, Controllable))
}

fn plant(sys: &mut FarmingSystem, world: &mut hecs::World, data: &DataStore, seed: &str) {
    *data.get::<Mutex<Option<String>>>("plant_request").unwrap().lock().unwrap() = Some(seed.to_string());
    sys.tick(world, 1.0, data);
}

fn notices(data: &DataStore) -> Vec<String> {
    std::mem::take(&mut *data.get::<Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap())
}

/// (tag, place) of every crop, sorted.
fn spots(world: &hecs::World) -> Vec<(String, u32)> {
    let mut v: Vec<(String, u32)> = world
        .query::<&CropInstance>()
        .iter()
        .map(|(_, c)| (c.tower_id.clone().unwrap_or_default(), c.tower_slot.unwrap_or(u32::MAX)))
        .collect();
    v.sort();
    v
}

/// Set the clock's speed (game seconds a real second).
fn speed(data: &DataStore, scale: f32) {
    data.get::<Mutex<crate::systems::time::GameTime>>("game_time").unwrap().lock().unwrap().time_scale = scale;
}

/// A SEED FROM THE BACKPACK GOES INTO THE NEAREST FREE PLACE IN REACH, where
/// the renderer draws it: the bed beside the player, its first plot, then its
/// second, and the seed leaves the backpack. Every crop it makes is tagged
/// with a published machine and a place below that machine's count, the two
/// things the renderer resolves a crop by (engine::home_meshes
/// `plan_plant_groups`); the old Plant button made a crop with neither.
///
/// Seen red with the crop spawned the old way, with no machine and no place
/// (`tower_id` and `tower_slot` None): both lettuces read ("", none).
#[test]
fn a_backpack_seed_goes_into_the_nearest_free_place_in_reach() {
    let mut data = home();
    stand(&mut data, 10.0, -1.5);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    let p = player(&mut world, &[("seed_lettuce_0", 3)]);
    plant(&mut sys, &mut world, &data, "seed_lettuce_0");
    plant(&mut sys, &mut world, &data, "seed_lettuce_0");
    assert_eq!(
        spots(&world),
        vec![("potato_grow_bed_1".to_string(), 0), ("potato_grow_bed_1".to_string(), 1)],
        "both lettuces went into the bed beside the player, one plot each"
    );
    assert_eq!(world.get::<&Inventory>(p).unwrap().count_item("seed_lettuce_0"), 1, "each took a seed");
    let said = notices(&data);
    assert!(said[0].starts_with("Planted lettuce in the potato grow bed beside you, plot 1 of 2."), "{said:?}");
    assert!(said[1].starts_with("Planted lettuce in the potato grow bed beside you, plot 2 of 2."), "{said:?}");
    // Drawable: a published machine (or the design its first tower owns) and a place inside it.
    let plots = data.get::<Vec<GrowPlot>>("grow_plots").unwrap();
    for (tag, slot) in spots(&world) {
        let plot = plots.iter().find(|g| g.id == tag || g.aliases.first() == Some(&tag)).expect("a published machine");
        let places = if plot.cups > 0 { plot.cups } else { 2 };
        assert!(slot < places, "{tag} place {slot} is inside the machine");
    }
    // The bed is full; the towers are 10 m off, out of reach: nothing is sown and the seed stays.
    plant(&mut sys, &mut world, &data, "seed_lettuce_0");
    assert_eq!(world.query::<&CropInstance>().iter().count(), 2, "nothing sown out of reach");
    assert_eq!(world.get::<&Inventory>(p).unwrap().count_item("seed_lettuce_0"), 1, "the seed stays in the backpack");
    assert_eq!(
        notices(&data),
        vec!["Nothing planted: you sow a seed where you stand, and no bed, tray, field or tower with room is within 5 m of you. \
              The nearest with room is the aeroponic tower, 10 m away."
            .to_string()]
    );
}

/// NOTHING WITH ROOM IN REACH PLANTS NOTHING, and says where the nearest is;
/// a machine on another deck is out of reach however close it is across the
/// floor.
///
/// Seen red with the reach test taken out of `choose` (every machine "in
/// reach", any deck): the lettuce was sown into the tower 10 m away.
#[test]
fn nothing_in_reach_plants_nothing_and_names_the_nearest() {
    let mut data = home();
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    let p = player(&mut world, &[("seed_lettuce_0", 2)]);
    stand(&mut data, 0.0, 30.0);
    plant(&mut sys, &mut world, &data, "seed_lettuce_0");
    assert_eq!(world.query::<&CropInstance>().iter().count(), 0, "nothing sown");
    assert_eq!(world.get::<&Inventory>(p).unwrap().count_item("seed_lettuce_0"), 2, "no seed taken");
    assert_eq!(
        notices(&data),
        vec!["Nothing planted: you sow a seed where you stand, and no bed, tray, field or tower with room is within 5 m of you. \
              The nearest with room is the aeroponic tower, 10 m away."
            .to_string()]
    );
    // Right above the bed, one deck up.
    data.insert("camera_position", Vec3::new(10.0, 5.0 + crate::ecs::components::EYE_HEIGHT_M as f32, -1.5));
    plant(&mut sys, &mut world, &data, "seed_lettuce_0");
    assert_eq!(world.query::<&CropInstance>().iter().count(), 0, "nothing sown from another deck");
    assert_eq!(
        notices(&data),
        vec!["Nothing planted: you sow a seed where you stand, and no bed, tray, field or tower with room is within 5 m of you. \
              The nearest with room is the potato grow bed, on another deck."
            .to_string()]
    );
}

/// GREEN SEEDS GO WHERE GREEN PLANTS GROW, SPAWN ONTO A MUSHROOM RACK, and
/// nothing into a fish tank. Standing between the rack (nearest), the tank
/// and the bed, a lettuce goes into the bed and an oyster spawn onto the rack.
///
/// Seen red with `choose` ignoring what a machine takes: the lettuce went
/// onto the rack's first shelf.
#[test]
fn each_seed_goes_only_where_its_kind_grows() {
    let mut data = home();
    stand(&mut data, 10.0, 3.5);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    player(&mut world, &[("seed_lettuce_0", 1), ("seed_oyster_mushroom_0", 1)]);
    plant(&mut sys, &mut world, &data, "seed_lettuce_0");
    plant(&mut sys, &mut world, &data, "seed_oyster_mushroom_0");
    let mut got: Vec<(String, String, u32)> = world
        .query::<&CropInstance>()
        .iter()
        .map(|(_, c)| (c.crop_def_id.clone(), c.tower_id.clone().unwrap_or_default(), c.tower_slot.unwrap_or(99)))
        .collect();
    got.sort();
    assert_eq!(
        got,
        vec![
            ("lettuce".to_string(), "potato_grow_bed_1".to_string(), 0),
            ("oyster_mushroom".to_string(), "mushroom_rack_1".to_string(), 0)
        ]
    );
    let said = notices(&data);
    assert!(said[1].starts_with("Planted oyster mushroom in the mushroom rack beside you, shelf 1 of 5."), "{said:?}");
}

/// The shipped grow media say what each machine takes: green plants for a
/// bed, a tray, a field and a tower, spawn for the mushroom rack, nothing for
/// the fish tank.
///
/// Seen red with `takes: Fungi` taken out of data/garden/grow_media.ron: the
/// rack read as taking plants.
#[test]
fn the_shipped_media_say_what_each_machine_takes() {
    let media = crate::systems::grow_machines::load_grow_media(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data"));
    let takes = |ty: &str| media.iter().find(|m| m.matches(ty)).map(|m| m.takes);
    assert_eq!(takes("mushroom_rack"), Some(Takes::Fungi));
    assert_eq!(takes("aquaponic_tank"), Some(Takes::NoSeeds));
    for ty in ["potato_grow_bed", "staple_grain_tray", "oilseed_bed", "grain_field", "tuber_field", "aeroponic_tower_nutrition"] {
        assert_eq!(takes(ty), Some(Takes::Plants), "{ty} takes green plants");
    }
}

/// A TOWER'S CUPS PLANTED UNDER ITS DESIGN ID COUNT AS TAKEN, and a seed sown
/// into the first tower of a design is tagged with that id, so the Garden
/// page's tower button then fills only the cup that is left: never two plants
/// in one cup. A seed at the second tower of the design is tagged with that
/// tower's own id.
///
/// Seen red with every place tagged by its machine id: the backpack lettuce
/// went into "ntower_0" cup 1, the cup the design's first planting already
/// drew a plant in.
#[test]
fn a_cup_taken_under_its_design_id_is_not_sown_twice() {
    let mut data = home();
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    player(&mut world, &[("seed_lettuce_0", 9)]);
    let tower_button = |data: &DataStore, cups: usize| {
        *data.get::<Mutex<Option<(String, Vec<String>)>>>("plant_tower_request").unwrap().lock().unwrap() =
            Some(("nutrition".to_string(), vec!["lettuce".to_string(); cups]));
    };
    tower_button(&data, 2);
    sys.tick(&mut world, 1.0, &data);
    stand(&mut data, 0.0, 1.0);
    plant(&mut sys, &mut world, &data, "seed_lettuce_0");
    tower_button(&data, 4);
    sys.tick(&mut world, 1.0, &data);
    let nutrition: Vec<u32> = spots(&world).into_iter().filter(|(t, _)| t == "nutrition").map(|(_, s)| s).collect();
    assert_eq!(nutrition, vec![0, 1, 2, 3], "one plant a cup: the button filled cup 4 only");
    assert_eq!(spots(&world).len(), 4, "nothing tagged with the first tower's own id");
    stand(&mut data, 0.0, 21.0);
    plant(&mut sys, &mut world, &data, "seed_lettuce_0");
    assert!(spots(&world).contains(&("ntower_1".to_string(), 0)), "the second tower's own id: {:?}", spots(&world));
}

/// A DEAD CROP GIVES UP ITS PLACE, and a home whose every place is growing
/// says so.
///
/// Seen red with dead crops counted as taken: the lettuce after the death was
/// refused (one living crop, not two), the bed counted full with a dead plant
/// in plot 1.
#[test]
fn a_dead_crop_gives_up_its_place_and_a_full_home_says_so() {
    let mut data = home();
    let bed: Vec<GrowPlot> = data.get::<Vec<GrowPlot>>("grow_plots").unwrap().iter().filter(|g| g.id == "potato_grow_bed_1").cloned().collect();
    data.insert("grow_plots", bed);
    stand(&mut data, 10.0, -1.5);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    player(&mut world, &[("seed_lettuce_0", 4)]);
    for _ in 0..3 {
        plant(&mut sys, &mut world, &data, "seed_lettuce_0");
    }
    assert_eq!(
        notices(&data).last().map(String::as_str),
        Some("Nothing planted: every bed, tray, field and tower cup already has a plant. Harvest what is ripe to make room.")
    );
    for (_, c) in world.query_mut::<&mut CropInstance>() {
        if c.tower_slot == Some(0) {
            c.growth_stage = STAGE_DEAD.to_string();
        }
    }
    plant(&mut sys, &mut world, &data, "seed_lettuce_0");
    let living: Vec<(String, u32)> =
        world.query::<&CropInstance>().iter().filter(|(_, c)| c.growth_stage != STAGE_DEAD).filter_map(|(_, c)| c.tower_id.clone().zip(c.tower_slot)).collect();
    assert_eq!(living.len(), 2, "the dead one was replaced");
    assert_eq!(world.query::<&CropInstance>().iter().count(), 2, "and cleared away");
}

/// A CROP SOWN FROM THE BACKPACK IS WATERED BY THE HOME'S IRRIGATION, the way
/// every crop in a machine is: twelve game hours on, it is still at the
/// irrigation's level and at full health.
///
/// Seen red with `WaterSupply::holds` answering None: the lettuce's water was
/// down to 0.018 after the twelve hours.
#[test]
fn a_backpack_crop_is_watered_by_the_home_irrigation() {
    let mut data = home();
    stand(&mut data, 10.0, -1.5);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    player(&mut world, &[("seed_lettuce_0", 1)]);
    plant(&mut sys, &mut world, &data, "seed_lettuce_0");
    // 720 game seconds a tick at the test clock's 72x, sixty ticks: twelve hours.
    for _ in 0..60 {
        sys.tick(&mut world, 10.0, &data);
    }
    let (_, c) = world.query::<&CropInstance>().iter().map(|(e, c)| (e, c.clone())).next().expect("the crop");
    assert!(c.water_level >= DEFAULT_AUTO_IRRIGATION - 0.05, "watered: {}", c.water_level);
    assert!(c.health > 99.0, "healthy: {}", c.health);
}

/// THE OUTLOOK SAYS HOW LONG A CROP TAKES, IN REAL TIME, AND WHAT ITS WATER
/// NEEDS. A lettuce (45 days) five days in has about 40 left: at the normal
/// 1x clock those are real days, at 72x about 13 real hours. Half its health
/// doubles what is left, as the tick grows it. Its water: the irrigation's,
/// or a hand watering's eight hours, or thirsty with two hours to live.
///
/// Seen red with the health left out of `ripe_in_game_s`: the lettuce at
/// half health read "about 45 days", not 90.
#[test]
fn the_outlook_says_the_wait_in_real_time_and_the_water_in_words() {
    let data = make_store();
    let reg = data.get::<PlantRegistry>("plant_registry").unwrap();
    let lettuce = reg.get("lettuce").unwrap();
    let day = SECONDS_PER_DAY;
    let crop = |age_days: f64, health: f32, water: f32| CropInstance {
        crop_def_id: "lettuce".into(),
        growth_stage: lettuce.first_stage().into(),
        planted_at: 100.0 * day - age_days * day,
        water_level: water,
        health,
        tower_id: Some("potato_grow_bed_1".into()),
        tower_slot: Some(0),
        health_seconds: 0.0,
        growing_seconds: 0.0,
    };
    let now = 100.0 * day;
    let o = outlook::outlook(&crop(5.0, 100.0, 1.0), Some(lettuce), now, 1.0, Some(0.8), true);
    assert_eq!(o.ripens, "about 40 days of real time");
    assert_eq!(o.tile, "40 days");
    assert_eq!(o.water, "The home's irrigation keeps it watered.");
    assert!(!o.thirsty);
    let fast = outlook::outlook(&crop(5.0, 100.0, 1.0), Some(lettuce), now, 72.0, Some(0.8), true);
    assert_eq!(fast.ripens, "about 40 days of game time, about 13 hours of real time at 72x speed");
    assert_eq!(fast.tile, "13 hours");
    let weak = outlook::outlook(&crop(0.0, 50.0, 1.0), Some(lettuce), now, 1.0, Some(0.8), true);
    assert_eq!(weak.ripens, "about 90 days of real time", "half the health, half the pace");
    let by_hand = outlook::outlook(&crop(5.0, 100.0, 1.0), Some(lettuce), now, 1.0, None, true);
    assert_eq!(by_hand.water, "Nothing waters it automatically: water it by hand before it dries out (a full watering lasts about 8 hours).");
    let drying = outlook::outlook(&crop(5.0, 100.0, 0.5), Some(lettuce), now, 1.0, None, true);
    assert_eq!(drying.water, "Nothing waters it automatically: water it by hand within about 3 hours (a full watering lasts about 8 hours).");
    let thirsty = outlook::outlook(&crop(5.0, 100.0, 0.1), Some(lettuce), now, 1.0, None, true);
    assert_eq!(thirsty.water, "Thirsty: water it now, or it dies in about 2 hours.");
    assert!(thirsty.thirsty);
    assert_eq!(thirsty.tile, "needs water");
    let ripe = CropInstance { growth_stage: lettuce.last_stage().into(), ..crop(45.0, 100.0, 1.0) };
    let ripe = outlook::outlook(&ripe, Some(lettuce), now, 1.0, Some(0.8), true);
    assert!(ripe.grown && ripe.ripens == "fully grown" && ripe.tile.is_empty(), "{ripe:?}");
    let dead = CropInstance { growth_stage: STAGE_DEAD.into(), ..crop(5.0, 0.0, 0.0) };
    assert_eq!(outlook::outlook(&dead, Some(lettuce), now, 1.0, None, true), CropOutlook::default());
    // The tick and the card read one water rule.
    let supply = WaterSupply { water_available: true, irrigation_on: true, targets: HashMap::new() };
    assert_eq!(supply.holds(Some("potato_grow_bed_1")), Some(DEFAULT_AUTO_IRRIGATION));
    assert_eq!(supply.holds(None), None, "a crop in no machine is watered by hand");
    assert_eq!(WaterSupply { irrigation_on: false, ..supply }.holds(Some("potato_grow_bed_1")), None);
}

/// PLANTING SAYS HOW LONG IT TAKES, HONESTLY. At the normal 1x clock a lettuce
/// is 45 days of real time, and the notice says so (crops are not sped up to
/// fit the first hour; the opening plants through the Garden page).
///
/// Seen red with the wait taken out of `planted_words`: "It ripens in ." was
/// not in the notice.
#[test]
fn planting_says_when_it_ripens_in_real_time() {
    let mut data = home();
    stand(&mut data, 10.0, -1.5);
    speed(&data, 1.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    player(&mut world, &[("seed_lettuce_0", 1)]);
    plant(&mut sys, &mut world, &data, "seed_lettuce_0");
    assert_eq!(
        notices(&data),
        vec!["Planted lettuce in the potato grow bed beside you, plot 1 of 2. It ripens in about 45 days of real time. \
              The home's irrigation keeps it watered. Look after it in Inventory > Garden."
            .to_string()]
    );
}

/// `choose` without a known position (a headless world) takes the first
/// machine with room, and a home with no place for a kind of seed says so.
///
/// Seen red with the last machine with room taken instead of the first:
/// ("potato_grow_bed_1", 0), not ("nutrition", 0).
#[test]
fn choose_without_a_position_and_with_no_place() {
    let data = home();
    let media = crate::systems::grow_machines::load_grow_media(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data"));
    let places = sowing::places(
        data.get::<Vec<GrowPlot>>("grow_plots").unwrap(),
        data.get::<HashMap<String, Vec<(String, u32)>>>("grow_instances"),
        &media,
    );
    let first = sowing::choose(&places, &Default::default(), Takes::Plants, None).unwrap();
    assert_eq!((first.tag.as_str(), first.slot, first.distance_m), ("nutrition", 0, None));
    let none = sowing::choose(&places[..2], &Default::default(), Takes::Fungi, None).unwrap_err();
    assert_eq!(
        sowing::refusal_words(&none),
        "Nothing planted: mushroom spawn grows on a mushroom rack, and there is none here."
    );
}

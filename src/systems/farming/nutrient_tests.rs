//! Nutrients as N, P2O5 and K2O through the real FarmingSystem tick
//! (2026-09-26, gardening depth rung 3). The model's pieces are unit-tested
//! in soil.rs; these drive whole ticks, so they prove the pieces are wired
//! into growth, health, fertilizing, the slider and replanting. Each was seen
//! red first; the doc comment on each says what was broken to see it.

use super::gardening_tests::make_store;
use super::soil::{self, NUTRIENT_HEALTH_FLOOR};
use super::*;
use crate::ecs::components::{Controllable, CropInstance, CropSoil, Irrigator, Npk};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::inventory::Inventory;

/// A well-watered crop of `plant` in unit 0 of grow area `area`, planted
/// `planted_at`, at `stage`.
fn crop(plant: &str, area: &str, stage: &str, planted_at: f64) -> CropInstance {
    CropInstance {
        crop_def_id: plant.to_string(),
        growth_stage: stage.to_string(),
        planted_at,
        water_level: 1.0,
        health: 100.0,
        tower_id: Some(area.to_string()),
        tower_slot: Some(0),
        health_seconds: 0.0,
        growing_seconds: 0.0,
    }
}

fn stages_of(data: &DataStore, plant: &str) -> Vec<String> {
    data.get::<PlantRegistry>("plant_registry")
        .unwrap()
        .get(plant)
        .unwrap()
        .stages()
        .iter()
        .map(|s| s.to_string())
        .collect()
}

fn store_of(world: &hecs::World, e: hecs::Entity) -> Npk {
    world.get::<&CropSoil>(e).expect("the crop has soil").store
}

fn health_of(world: &hecs::World, e: hecs::Entity) -> f32 {
    world.get::<&CropInstance>(e).unwrap().health
}

fn notices(data: &DataStore) -> Vec<String> {
    std::mem::take(&mut *data.get::<std::sync::Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap())
}

fn set_fertilize(data: &DataStore, e: hecs::Entity) {
    *data.get::<std::sync::Mutex<Option<u64>>>("fertilize_crop_request").unwrap().lock().unwrap() =
        Some(e.to_bits().into());
}

/// The shipped compost bag's plant-available grams (1.316 N, 6.4 P2O5,
/// 11.6 K2O), worked out from the shipped data the way the tick does.
fn compost_bag() -> Npk {
    let data = soil::NutrientData::parse(soil::NUTRIENTS_RON).unwrap();
    data.fertilizer("fertilizer_0").unwrap().available_per_item(2.0)
}

/// The law of the minimum through the tick: two tomatoes side by side, both
/// watered, one with plenty of everything and one with plenty of N and K
/// but NO phosphorus. The first stays at full health; the second eases down
/// (gradually, never to death, never below the floor), the player is told
/// once which nutrient is short, and one bag of compost brings it back.
/// Seen red by making `sufficiency` take the MAXIMUM of the three instead of
/// the minimum: the P-starved crop then stayed at 100.
#[test]
fn a_crop_short_of_one_nutrient_is_stunted_by_the_minimum_rule() {
    let mut data = make_store();
    data.insert("crop_growth_speed", std::sync::Mutex::new(1.0_f32));
    data.insert("player_notices", std::sync::Mutex::new(Vec::<String>::new()));
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    let mut inv = Inventory::new(8);
    inv.add_item("fertilizer_0", 1, 99);
    world.spawn((inv, Controllable));
    let first = stages_of(&data, "tomato")[0].clone();
    let plenty = Npk::new(10.0, 10.0, 10.0);
    let no_p = Npk::new(10.0, 0.0, 10.0);
    let fed = world.spawn((crop("tomato", "bed_a", &first, 0.0), CropSoil { store: plenty, uptake: 0.0 }));
    let starved = world.spawn((crop("tomato", "bed_b", &first, 0.0), CropSoil { store: no_p, uptake: 0.0 }));

    for _ in 0..300 {
        sys.tick(&mut world, 1.0, &data);
    }
    assert!(health_of(&world, fed) >= 99.9, "fully supplied: {}", health_of(&world, fed));
    let after_300 = health_of(&world, starved);
    assert!(
        after_300 < 75.0 && after_300 > NUTRIENT_HEALTH_FLOOR,
        "no phosphorus, plenty of N and K: health eases down, got {after_300}"
    );
    assert!(after_300 > 100.0 - 300.0 * 0.2, "gradually, not all at once: {after_300}");
    let said = notices(&data);
    assert_eq!(said.len(), 1, "told once: {said:?}");
    assert!(said[0].contains("bed b") && said[0].contains("phosphorus"), "{}", said[0]);

    // A long shortage stunts but never kills.
    for _ in 0..2000 {
        sys.tick(&mut world, 1.0, &data);
    }
    let floor = health_of(&world, starved);
    assert!((floor - NUTRIENT_HEALTH_FLOOR).abs() < 1e-3, "held at the floor, got {floor}");
    assert_ne!(world.get::<&CropInstance>(starved).unwrap().growth_stage, STAGE_DEAD);
    assert!(notices(&data).is_empty(), "not told again while it stays short");

    // One bag of compost and it recovers.
    set_fertilize(&data, starved);
    for _ in 0..200 {
        sys.tick(&mut world, 1.0, &data);
    }
    assert!(health_of(&world, starved) >= 99.9, "fertilized, it recovers: {}", health_of(&world, starved));
}

/// Fertilizing puts one bag's plant-available nutrients into the crop's
/// unit, in the compost's own ratio (WSU: 1.3 g N, 6.4 g P2O5, 11.6 g K2O a
/// bag), takes the bag, and does NOT also add health the way the old model
/// did (+40): the health follows the soil, so counting both would feed the
/// crop twice. A ripe crop is used so no growth draw muddies the grams.
/// Seen red by restoring the old `health + 40` line.
#[test]
fn fertilizing_adds_a_bags_nutrients_at_the_cited_ratio_and_nothing_else() {
    let data = make_store();
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    let mut inv = Inventory::new(8);
    inv.add_item("fertilizer_0", 2, 99);
    let player = world.spawn((inv, Controllable));
    let ripe = stages_of(&data, "tomato").last().unwrap().clone();
    let mut c = crop("tomato", "bed_a", &ripe, 0.0);
    c.health = 40.0;
    let e = world.spawn((c, CropSoil { store: Npk::ZERO, uptake: 1.0 }));

    set_fertilize(&data, e);
    sys.tick(&mut world, 1.0, &data);

    assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("fertilizer_0"), 1, "one bag used");
    let got = store_of(&world, e);
    let bag = compost_bag();
    assert!((got.n - 1.316).abs() < 1e-9 && (got.n - bag.n).abs() < 1e-12, "N {}", got.n);
    assert!((got.p2o5 - 6.4).abs() < 1e-9, "P2O5 {}", got.p2o5);
    assert!((got.k2o - 11.6).abs() < 1e-9, "K2O {}", got.k2o);
    assert!(
        (got.p2o5 / got.n - 0.32 / (0.94 * 0.07)).abs() < 1e-9,
        "in the compost's ratio: {} g P2O5 per g of available N",
        got.p2o5 / got.n
    );
    assert_eq!(health_of(&world, e), 40.0, "fertilizing feeds the soil, not the health bar");

    // No bag, no nutrients.
    set_fertilize(&data, e);
    sys.tick(&mut world, 1.0, &data);
    set_fertilize(&data, e);
    sys.tick(&mut world, 1.0, &data);
    assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("fertilizer_0"), 0);
    let twice = store_of(&world, e);
    assert!((twice.n - 2.0 * bag.n).abs() < 1e-9, "the second bag landed, the third did not exist: {}", twice.n);
}

/// A crop's draw follows its plants.csv demand: a tomato twin with every
/// index doubled draws exactly twice as much of each nutrient over the same
/// growth, and the real tomato draws its cited season removal (1.5 g N,
/// 0.9 g P2O5, 4.0 g K2O) times the share of the season it has grown.
/// Seen red by making `season_need` use a flat index of 1 for every crop
/// instead of the crop's own: the twin then drew the same as the tomato.
#[test]
fn nutrient_draw_scales_with_the_plants_csv_demand() {
    let mut data = make_store();
    data.insert("crop_growth_speed", std::sync::Mutex::new(1.0_f32));
    // The twin: the tomato row with every nutrient index doubled.
    let mut plants = PlantRegistry::from_csv(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/plants.csv")))
        .expect("plants.csv");
    let mut twin = plants.get("tomato").unwrap().clone();
    twin.id = "hungry_tomato".to_string();
    twin.nutrient_n *= 2.0;
    twin.nutrient_p *= 2.0;
    twin.nutrient_k *= 2.0;
    plants.plants.insert(twin.id.clone(), twin);
    data.insert("plant_registry", plants);

    // 30% of the way through the tomato's season, in one tick.
    let growth_seconds = 70.0 * SECONDS_PER_DAY;
    data.get::<std::sync::Mutex<crate::systems::time::GameTime>>("game_time")
        .unwrap()
        .lock()
        .unwrap()
        .elapsed_seconds = growth_seconds * 0.3;
    let first = stages_of(&data, "tomato")[0].clone();
    let plenty = Npk::new(100.0, 100.0, 100.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    let tomato = world.spawn((crop("tomato", "bed_a", &first, 0.0), CropSoil { store: plenty, uptake: 0.0 }));
    let hungry = world.spawn((crop("hungry_tomato", "bed_b", &first, 0.0), CropSoil { store: plenty, uptake: 0.0 }));
    sys.tick(&mut world, 1.0, &data);

    let drawn = |e| {
        let s = store_of(&world, e);
        Npk::new(100.0 - s.n, 100.0 - s.p2o5, 100.0 - s.k2o)
    };
    let (t, h) = (drawn(tomato), drawn(hungry));
    assert!(t.n > 0.0 && t.p2o5 > 0.0 && t.k2o > 0.0, "the tomato drew something: {t:?}");
    for (name, a, b) in [("N", t.n, h.n), ("P2O5", t.p2o5, h.p2o5), ("K2O", t.k2o, h.k2o)] {
        assert!((b / a - 2.0).abs() < 1e-9, "{name}: twice the index draws twice ({b} vs {a})");
    }
    // The tomato's draw is its season removal times its uptake share. The
    // tick ran in daylight (the default clock sits at 08:00 with the clock
    // itself not moving), so light moved its planted_at one second back:
    // read the share from the crop's actual age.
    let age = crate::systems::time::elapsed_now(&data) - world.get::<&CropInstance>(tomato).unwrap().planted_at;
    let share = f64::from(soil::uptake_fraction((age / growth_seconds) as f32, stages_of(&data, "tomato").len()));
    assert!((t.n - 1.5 * share).abs() < 1e-5, "N {} vs 1.5 x {share}", t.n);
    assert!((t.p2o5 - 0.9 * share).abs() < 1e-5, "P2O5 {} vs 0.9 x {share}", t.p2o5);
    assert!((t.k2o - 4.0 * share).abs() < 1e-5, "K2O {} vs 4.0 x {share}", t.k2o);
}

/// Nothing is counted twice with the old slider. Until this rung the
/// "nutrient" slider multiplied growth (1.0 gave 1.5x, 0.0 gave 0.5x). Now,
/// with the soil well supplied, a crop under a slider at 1.0 and one at 0.0
/// grow exactly alike: the slider's only effect is the feeder. Seen red by
/// restoring the old `0.5 + n` growth multiplier.
#[test]
fn the_nutrient_slider_no_longer_speeds_growth() {
    let mut data = make_store();
    data.insert("crop_growth_speed", std::sync::Mutex::new(1.0_f32));
    let mut nut = HashMap::new();
    nut.insert("rich".to_string(), 1.0_f32);
    nut.insert("lean".to_string(), 0.0_f32);
    data.insert("garden_nutrient", std::sync::Mutex::new(nut));
    // 60% of the tomato's window: the old multipliers put these two crops
    // two stages apart here.
    data.get::<std::sync::Mutex<crate::systems::time::GameTime>>("game_time")
        .unwrap()
        .lock()
        .unwrap()
        .elapsed_seconds = 70.0 * SECONDS_PER_DAY * 0.6;
    let first = stages_of(&data, "tomato")[0].clone();
    let plenty = CropSoil { store: Npk::new(100.0, 100.0, 100.0), uptake: 0.0 };
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    let rich = world.spawn((crop("tomato", "rich", &first, 0.0), plenty.clone()));
    let lean = world.spawn((crop("tomato", "lean", &first, 0.0), plenty));
    sys.tick(&mut world, 1.0, &data);
    let (r, l) = (
        world.get::<&CropInstance>(rich).unwrap().growth_stage.clone(),
        world.get::<&CropInstance>(lean).unwrap().growth_stage.clone(),
    );
    assert_ne!(r, first, "both grew");
    assert_eq!(r, l, "the slider no longer changes how fast a supplied crop grows");
}

/// The slider now runs a FEEDER that spends real fertilizer. Two empty units
/// side by side: the one whose slider is at 1.0 opens a bag from home
/// storage and is topped up to two reserves, and keeps its crop healthy;
/// the one at 0.0 gets nothing and goes short. Out of stock in survival the
/// feeder says so once; in creative it feeds for free. Seen red by skipping
/// the feeder's top-up: the fed unit stayed empty and its crop went short.
#[test]
fn the_nutrient_slider_runs_a_feeder_that_spends_stored_fertilizer() {
    let run = |creative: bool, bags: u32| {
        let mut data = make_store();
        data.insert("crop_growth_speed", std::sync::Mutex::new(1.0_f32));
        data.insert("creative_mode", std::sync::Mutex::new(creative));
        data.insert("player_notices", std::sync::Mutex::new(Vec::<String>::new()));
        let mut stock = HashMap::new();
        stock.insert("fertilizer_0".to_string(), bags);
        data.insert("home_stock", std::sync::Mutex::new(stock));
        let mut nut = HashMap::new();
        nut.insert("fed".to_string(), 1.0_f32);
        nut.insert("unfed".to_string(), 0.0_f32);
        data.insert("garden_nutrient", std::sync::Mutex::new(nut));
        let first = stages_of(&data, "tomato")[0].clone();
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        world.spawn((Irrigator,));
        let empty = CropSoil { store: Npk::ZERO, uptake: 0.0 };
        let fed = world.spawn((crop("tomato", "fed", &first, 0.0), empty.clone()));
        let unfed = world.spawn((crop("tomato", "unfed", &first, 0.0), empty));
        sys.tick(&mut world, 1.0, &data);
        let after_one = (store_of(&world, fed), store_of(&world, unfed));
        for _ in 0..300 {
            sys.tick(&mut world, 1.0, &data);
        }
        let left = *data
            .get::<std::sync::Mutex<HashMap<String, u32>>>("home_stock")
            .unwrap()
            .lock()
            .unwrap()
            .get("fertilizer_0")
            .unwrap();
        (after_one, health_of(&world, fed), health_of(&world, unfed), left, notices(&data))
    };

    // Survival, three bags in the Barn.
    let ((fed1, unfed1), fed_h, unfed_h, left, said) = run(false, 3);
    let need_n = 1.5; // the tomato's season N (soil.rs pins it)
    assert!(
        fed1.n > 0.9 * soil::feed_target(Npk::new(need_n, 0.9, 4.0), 1.0).n,
        "topped up to about two reserves of N: {fed1:?}"
    );
    assert!(fed1.p2o5 > 0.0 && fed1.k2o > 0.0, "with the compost's P and K along with it");
    assert_eq!(unfed1, Npk::ZERO, "the unfed unit got nothing");
    assert_eq!(left, 2, "the feeder opened exactly one bag from home storage");
    assert!(fed_h >= 99.9, "the fed crop stays healthy: {fed_h}");
    assert!(unfed_h < 90.0, "the unfed one goes short: {unfed_h}");
    assert!(said.iter().any(|s| s.contains("unfed")), "and the player is told: {said:?}");
    assert!(!said.iter().any(|s| s.contains("feeder")), "the feeder had stock: {said:?}");

    // Survival with an empty Barn: nothing to feed with, and it says so once.
    let ((fed1, _), fed_h, _, left, said) = run(false, 0);
    assert_eq!(fed1, Npk::ZERO, "no fertilizer, no feeding");
    assert!(fed_h < 90.0, "so the fed area goes short too: {fed_h}");
    assert_eq!(left, 0);
    assert_eq!(said.iter().filter(|s| s.contains("feeder")).count(), 1, "told once: {said:?}");

    // Creative: fed for free.
    let ((fed1, _), fed_h, _, left, _) = run(true, 0);
    assert!(fed1.n > 0.0 && fed_h >= 99.9, "creative feeds without stock");
    assert_eq!(left, 0, "and takes nothing");
}

/// Cropping a unit mines it: what a crop leaves in its unit is what the
/// next crop sown there starts on, not fresh soil. Seen red by not
/// remembering the soil at harvest: the replanted crop started fresh.
#[test]
fn the_next_crop_in_a_unit_starts_on_what_the_last_one_left() {
    let mut data = make_store();
    data.insert("creative_mode", std::sync::Mutex::new(true));
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Inventory::new(16), Controllable));
    let sow = |data: &DataStore| {
        *data.get::<std::sync::Mutex<Option<(String, String, u32)>>>("plant_bed_request").unwrap().lock().unwrap() =
            Some(("potato_grow_bed".to_string(), "potato".to_string(), 1));
    };
    sow(&data);
    sys.tick(&mut world, 1.0, &data);
    let first: Vec<hecs::Entity> = world.query::<&CropInstance>().iter().map(|(e, _)| e).collect();
    assert_eq!(first.len(), 1);
    let fresh = store_of(&world, first[0]);
    assert!(fresh.n > 0.0, "the first crop started on fresh soil: {fresh:?}");

    // The first crop has worked its unit down; ripen and pick it.
    let worked = Npk::new(0.01, 0.02, 0.03);
    world.get::<&mut CropSoil>(first[0]).unwrap().store = worked;
    *data.get::<std::sync::Mutex<bool>>("dev_grow_crops").unwrap().lock().unwrap() = true;
    sys.tick(&mut world, 1.0, &data);
    *data.get::<std::sync::Mutex<Option<u64>>>("harvest_request").unwrap().lock().unwrap() =
        Some(first[0].to_bits().into());
    sys.tick(&mut world, 1.0, &data);
    assert_eq!(world.query::<&CropInstance>().iter().count(), 0, "harvested");

    // Sow again: the new crop starts where the old one left off.
    sow(&data);
    sys.tick(&mut world, 1.0, &data);
    let second: Vec<hecs::Entity> = world.query::<&CropInstance>().iter().map(|(e, _)| e).collect();
    assert_eq!(second.len(), 1);
    let got = store_of(&world, second[0]);
    assert!(got.n <= worked.n && got.p2o5 <= worked.p2o5 && got.k2o <= worked.k2o, "worked soil, not fresh: {got:?}");
    assert!(got.n < fresh.n / 10.0, "far below fresh ({:?})", fresh);
}

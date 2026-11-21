//! The harvest basket through the real harvest (2026-11-21, basket.rs): the
//! FarmingSystem tick, the shipped plants.csv and items.csv, a harvest press.
//! Each test was seen red first; its doc comment says how.

use std::collections::HashMap;
use std::sync::Mutex;

use super::basket::{self, HarvestBasket};
use super::fixtures::gardening::make_store;
use super::units::PLOT_AREA_KEY;
use super::*;
use humanity_core::ecs::components::{Controllable, CropInstance};
use humanity_core::ecs::systems::System;
use humanity_core::hot_reload::data_store::DataStore;
use crate::inventory::Inventory;

const RADISH: &str = "vegetable_radish_0";
const LETTUCE: &str = "vegetable_lettuce_0";

/// The shipped store, a lettuce bed plot of 1 m2 (fourteen heads), the
/// notices, no pests, and creative mode so no seed comes back to muddle the
/// pack.
fn store() -> DataStore {
    let mut data = make_store();
    let areas: HashMap<String, f32> = [("lettuce_bed".to_string(), 1.0_f32)].into_iter().collect();
    data.insert(PLOT_AREA_KEY, areas);
    crate::notices::register(&mut data);
    data.insert("garden_pest_severity", Mutex::new(0.0_f32));
    data.insert("creative_mode", Mutex::new(true));
    // Time speed 1: the few hundred one-second ticks below are minutes of
    // garden time, nothing a ripe crop notices.
    data.get::<Mutex<crate::time::GameTime>>("game_time").unwrap().lock().unwrap().time_scale = 1.0;
    data
}

/// A ripe crop of `plant` that spent its whole season at full health, in unit
/// 0 of `area` (None: hand-planted, one plant).
fn ripe(data: &DataStore, plant: &str, area: Option<&str>) -> CropInstance {
    let last = data.get::<PlantRegistry>("plant_registry").unwrap().get(plant).unwrap().last_stage().to_string();
    CropInstance {
        crop_def_id: plant.to_string(),
        growth_stage: last,
        planted_at: 0.0,
        water_level: 1.0,
        health: 100.0,
        tower_id: area.map(str::to_string),
        tower_slot: area.map(|_| 0),
        health_seconds: 1000.0,
        growing_seconds: 1000.0,
    }
}

/// A player whose pack never fills.
fn player(world: &mut hecs::World) -> hecs::Entity {
    let mut inv = Inventory::new(64);
    inv.volume_capacity_l = 1.0e9;
    world.spawn((inv, Controllable))
}

fn count(world: &hecs::World, who: hecs::Entity, item: &str) -> u32 {
    world.get::<&Inventory>(who).unwrap().count_item(item)
}

fn kept(world: &hecs::World, item: &str) -> f64 {
    basket::of_player(world).toward(item)
}

fn press(sys: &mut FarmingSystem, world: &mut hecs::World, data: &DataStore, e: hecs::Entity) {
    *data.get::<Mutex<Option<u64>>>("harvest_request").unwrap().lock().unwrap() = Some(e.to_bits().into());
    sys.tick(world, 1.0, data);
}

/// A HARVEST UNDER ONE ITEM IS KEPT, AND THE ITEM COMES WHEN THE GRAMS REACH
/// IT. One radish tower cup yields 0.0079 to 0.0273 of a 300 g Radish
/// (plants.csv, items.csv). The first cup gives no Radish and its grams are
/// kept, within the cup's range; every cup after adds its grams, and a Radish
/// arrives on exactly the harvest that brings the kept grams to a whole one,
/// never before and never more; over the whole run, the Radishes handed over
/// plus what is kept equal what was picked to the gram. Runs until two whole
/// Radishes have come (at most 2 / 0.0079, about 254 cups). The press's line
/// says what was picked and what is kept toward the next.
///
/// Seen red 2026-11-21 with the basket in place but the harvest still
/// rounding probabilistically (the code before this rung): "the first cup's
/// grams are kept (0.0079 to 0.0273 of a Radish), kept 0".
#[test]
fn a_harvest_under_one_item_is_kept_until_the_grams_make_one() {
    let data = store();
    let def = data.get::<PlantRegistry>("plant_registry").unwrap().get("radish").unwrap().clone();
    let (lo, hi) = (f64::from(def.yield_min), f64::from(def.yield_max));
    assert!(hi < 1.0, "a radish cup yields under one Radish (plants.csv), else this test proves nothing");
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    let who = player(&mut world);
    let cups: Vec<hecs::Entity> = (0..300).map(|_| world.spawn((ripe(&data, "radish", None),))).collect();

    press(&mut sys, &mut world, &data, cups[0]);
    assert!(world.get::<&CropInstance>(cups[0]).is_err(), "the cup was harvested");
    assert_eq!(count(&world, who, RADISH), 0, "one cup makes no whole Radish");
    let first = kept(&world, RADISH);
    assert!(
        first >= lo - 1e-6 && first <= hi + 1e-6,
        "the first cup's grams are kept ({lo:.4} to {hi:.4} of a Radish), kept {first}"
    );
    let said = crate::notices::take_said(&data);
    assert!(
        said.iter().any(|l| l.contains("g of radish") && l.contains("kept toward it") && l.contains("300 g")),
        "the press says what it picked and what is kept: {said:?}"
    );

    let mut picked = first;
    let mut arrivals = 0;
    for &cup in &cups[1..] {
        let (items_before, kept_before) = (count(&world, who, RADISH), kept(&world, RADISH));
        press(&mut sys, &mut world, &data, cup);
        let (items_after, kept_after) = (count(&world, who, RADISH), kept(&world, RADISH));
        let came = items_after - items_before;
        let got = f64::from(came) + kept_after - kept_before;
        assert!(got >= lo - 1e-6 && got <= hi + 1e-6, "each cup picks within its range: {got}");
        assert_eq!(came, (kept_before + got + 1e-9).floor() as u32, "a Radish comes exactly when the kept grams reach one");
        assert!(kept_after < 1.0, "never a whole Radish left in the basket: {kept_after}");
        picked += got;
        if came > 0 {
            arrivals += 1;
            let said = crate::notices::take_said(&data);
            assert!(
                said.iter().any(|l| l.contains("1 whole (300 g each)")),
                "the press that makes a Radish says so: {said:?}"
            );
        }
        if count(&world, who, RADISH) >= 2 {
            break;
        }
    }
    assert_eq!(count(&world, who, RADISH), 2, "two whole Radishes came from the cups");
    assert_eq!(arrivals, 2, "one at a time");
    let total = f64::from(count(&world, who, RADISH)) + kept(&world, RADISH);
    assert!((total - picked).abs() < 1e-9, "nothing lost or made: {total} handed over and kept, {picked} picked");
}

/// A HARVEST OF SEVERAL ITEMS GIVES WHOLE ITEMS AND KEEPS THE REST. A 1 m2
/// lettuce plot holds fourteen heads at 1.085 items each (plants.csv, no range
/// to roll): 15.19 items, so 15 whole and 0.19 kept; a second plot adds its
/// 15.19 to the 0.19, so 15 more and 0.38 kept. Exact, not a dice roll.
///
/// Seen red 2026-11-21 on the code before this rung (probabilistic rounding,
/// no basket): "the first plot keeps its fraction: kept 0, want 0.19" (and
/// the count was 15 or 16 by chance).
#[test]
fn a_harvest_of_several_items_gives_whole_items_and_keeps_the_rest() {
    let data = store();
    let per_plant = f64::from(data.get::<PlantRegistry>("plant_registry").unwrap().get("lettuce").unwrap().yield_max);
    let plot = 14.0 * per_plant;
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    let who = player(&mut world);
    let a = world.spawn((ripe(&data, "lettuce", Some("lettuce_bed")),));
    press(&mut sys, &mut world, &data, a);
    assert_eq!(count(&world, who, LETTUCE), plot.floor() as u32, "the plot's whole heads ({plot:.3} items)");
    let want = plot.fract();
    assert!((kept(&world, LETTUCE) - want).abs() < 1e-6, "the first plot keeps its fraction: kept {}, want {want:.2}", kept(&world, LETTUCE));
    let said = crate::notices::take_said(&data);
    assert!(
        said.iter().any(|l| l.contains("of lettuce") && l.contains("15 whole") && l.contains("kept toward the next")),
        "the press says the whole heads and what is kept: {said:?}"
    );

    let b = world.spawn((ripe(&data, "lettuce", Some("lettuce_bed")),));
    press(&mut sys, &mut world, &data, b);
    let both = 2.0 * plot;
    assert_eq!(count(&world, who, LETTUCE), both.floor() as u32, "the second plot's fraction added to the first's");
    assert!((kept(&world, LETTUCE) - both.fract()).abs() < 1e-6, "kept {}", kept(&world, LETTUCE));
}

/// The basket belongs to the player and to no one else: a world without a
/// player keeps no basket anywhere, and `restore` replaces the player's (an
/// empty save empties it), so a load never mixes two worlds' grams.
#[test]
fn restore_replaces_the_players_basket() {
    let mut world = hecs::World::new();
    assert_eq!(basket::of_player(&world), HarvestBasket::default(), "no player, no basket");
    let who = player(&mut world);
    let mut saved = HarvestBasket::default();
    saved.add(RADISH, 0.375);
    basket::restore(&mut world, &saved);
    assert_eq!(basket::of_player(&world), saved);
    basket::restore(&mut world, &HarvestBasket::default());
    assert_eq!(basket::of_player(&world), HarvestBasket::default(), "an empty save empties it");
    assert!(world.get::<&HarvestBasket>(who).is_ok(), "on the player");
}

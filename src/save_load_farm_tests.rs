//! THE SAVE AND THE FARM'S OWN CLOCK (2026-10-31, docs/design/farm-clock.md rung 2, the seam
//! review's findings A and B).
//!
//! Each grow area advances on its own turn, so at any moment most areas owe some game time. The
//! save used to bring every area and the air to now first (`farming::settle(.., 0)`): the whole
//! farm on the save's frame (every autosave and every editor keep-edits), and its fertilizer draws
//! made outside the frame's home-storage window, so they never left the containers. Now the save
//! writes the garden as it stands with what each area and the air still owe (`WorldSave::
//! farm_owed`), and the load owes it again. These tests go through the game's own save and load
//! (`save_home_at`, `apply_save_to_world`, `resume_home`) with the game's shared farm in the store:
//! without it the save would read nothing owed and the load would differ, so a store that lacks
//! the farm cannot pass here the way the slot tests' stores (no farm at all) passed silently.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::*;
use humanity_core::ecs::systems::System;
use humanity_systems::farming::{self, fixtures::step, FarmingSystem};

/// The step_tests garden (every regime crossed, no air), the game's shared farm at an hour's
/// interval, and no feeder setting: the feeder's opened bags are not saved (a reload loses at most
/// one part-used item of each kind), which these tests are not about.
fn farm_home() -> (humanity_core::hot_reload::data_store::DataStore, hecs::World, farming::Shared) {
    let (mut data, world) = step::garden(false);
    // The farm clock's channels as the game registers them at boot (the load's hand-over arrives
    // through one of them), then an hour's interval.
    farming::clock::register(&mut data);
    data.insert(farming::clock::INTERVAL_KEY, Mutex::new(3600.0_f64));
    data.insert("garden_nutrient", Mutex::new(HashMap::<String, f32>::new()));
    let mut sys = FarmingSystem::new();
    sys.reuse_override = Some(true);
    let shared: farming::Shared = Arc::new(Mutex::new(sys));
    data.insert(farming::SHARED_KEY, shared.clone());
    (data, world, shared)
}

fn clock_now(data: &humanity_core::hot_reload::data_store::DataStore) -> f64 {
    data.get::<Mutex<humanity_systems::time::GameTime>>("game_time").unwrap().lock().unwrap().elapsed_seconds
}

fn set_clock(data: &humanity_core::hot_reload::data_store::DataStore, t: f64) {
    data.get::<Mutex<humanity_systems::time::GameTime>>("game_time").unwrap().lock().unwrap().set_elapsed(t);
}

fn tick(data: &humanity_core::hot_reload::data_store::DataStore, world: &mut hecs::World, shared: &farming::Shared, dt: f32) {
    set_clock(data, clock_now(data) + f64::from(dt));
    shared.lock().unwrap().tick(world, dt, data);
}

/// How two ends differ by more than `tol` (relative, or absolute for a value under 1): every stage
/// must be the same.
fn apart(a: &step::Ended, b: &step::Ended, tol: f64) -> Vec<String> {
    let (stages, worst) = step::compare(a, b);
    let mut out = stages;
    out.extend(worst.into_iter().filter(|(_, w)| w.abs > tol && w.rel > tol).map(|(k, w)| format!("{k}: {} ({} relative) at {}", w.abs, w.rel, w.at)));
    out
}

/// No difference at all: every stage the same and every quantity to the bit.
fn same(a: &step::Ended, b: &step::Ended) -> Vec<String> {
    apart(a, b, 0.0)
}

/// A SAVE KEEPS WHAT EACH AREA OWES, AND A LOAD OWES IT AGAIN. Between their turns the areas owe
/// up to four minutes; the save moves nothing (no crop, no clock) and writes what each owes; the
/// save loaded into a second world and its first frame end where the first world ends when it is
/// brought to now: every stage the same and every quantity to 1e-12 (the loaded world holds its
/// crops in another order, so a sum over crops rounds differently in the last bits). Without the
/// owed time handed over the bed's crops ended four minutes behind (seen red 2026-10-31, the second
/// store lacking the clock's channels: "crop.growing_seconds: 240"); with the save not writing it,
/// "the save carries what each area owed".
#[test]
fn a_save_keeps_what_each_area_owes_and_a_load_owes_it_again() {
    let (data, mut world, shared) = farm_home();
    tick(&data, &mut world, &shared, 1.0);
    for _ in 0..4 {
        tick(&data, &mut world, &shared, 60.0);
    }
    let owed = farming::owed(&data).expect("the game's farm is in the store");
    assert!(owed.areas.values().any(|s| *s > 100.0), "areas owe time between their turns: {owed:?}");

    let before = step::ended_by_place(&world);
    let root = humanity_core::test_temp::dir("farm_owed_save");
    let path = root.join("saves").join(persistence::OFFLINE_SAVE_FILE);
    save_home_at(&path, &world, &[], &data, true, None);
    let moved = same(&before, &step::ended_by_place(&world));
    assert!(moved.is_empty(), "the save moved the garden: {moved:?}");
    assert_eq!(farming::owed(&data).as_ref(), Some(&owed), "the save moved the farm's clock");
    let saved = persistence::load_world(&path).expect("the save reads back");
    assert_eq!(saved.farm_owed, owed, "the save carries what each area owed");

    // The load, as the game does it: the save applied to a world and resumed, then its first frame.
    let (data2, mut world2, shared2) = farm_home();
    set_clock(&data2, clock_now(&data));
    apply_save_to_world(&mut world2, &saved);
    resume_home(&mut world2, &data2, &saved, false, 1.0, None);
    shared2.lock().unwrap().tick(&mut world2, 0.0, &data2);
    // The first world, brought to now.
    farming::settle(&mut world, &data, 0.0);
    let differ = apart(&step::ended_by_place(&world), &step::ended_by_place(&world2), 1e-12);
    assert!(differ.is_empty(), "the loaded garden's first frame is not the saved one brought to now: {differ:?}");
}

/// A SETTLE BETWEEN A LOAD AND THE FARM'S NEXT TICK ADVANCES NOTHING (2026-10-31, the critic's F3).
/// A load in the running game (the save picker, a snapshot restore) applies the save to the world
/// and hands its debts to the farm (`clock::hand_over_resume`), which takes them at its next tick;
/// a reader asking in between (the plant meshes early in the frame, a wall screen's Garden data)
/// found the PREVIOUS world's clock still in place and advanced every area whose name the loaded
/// home shares (a restore of the same home shares all) by that clock's old debt, on the loaded
/// world, and then the tick owed the saved debt again: time that belonged to no world. Now every
/// settle, and what it says it would do, waits for the hand-over, and the save in between writes
/// the loaded debts. Seen red 2026-10-31 on the code before the fix: "a settle with the
/// hand-over still waiting would advance nothing".
#[test]
fn a_settle_between_a_load_and_the_farms_next_tick_advances_nothing() {
    let (data, mut world, shared) = farm_home();
    tick(&data, &mut world, &shared, 1.0);
    for _ in 0..4 {
        tick(&data, &mut world, &shared, 60.0);
    }
    let root = humanity_core::test_temp::dir("farm_owed_resume");
    let path = root.join("saves").join(persistence::OFFLINE_SAVE_FILE);
    save_home_at(&path, &world, &[], &data, true, None);
    let saved = persistence::load_world(&path).expect("the save reads back");
    // Time goes on, then the save is loaded onto the running game: the same store, the same farm.
    for _ in 0..3 {
        tick(&data, &mut world, &shared, 60.0);
    }
    apply_save_to_world(&mut world, &saved);
    resume_home(&mut world, &data, &saved, false, 1.0, None);
    let loaded = step::ended_by_place(&world);
    assert!(!farming::settle_needed(&data, 0.0), "a settle with the hand-over still waiting would advance nothing");
    farming::settle(&mut world, &data, 0.0);
    farming::settle_areas(&mut world, &data, &["bed_a".to_string(), "bed_b".to_string()], 1.0);
    let moved = same(&loaded, &step::ended_by_place(&world));
    assert!(moved.is_empty(), "a settle before the farm's next tick moved the loaded garden: {moved:?}");
    assert_eq!(farming::owed(&data).as_ref(), Some(&saved.farm_owed), "a save before the tick writes the loaded debts");
}

/// THE HARVEST BASKET SURVIVES A SAVE AND A LOAD (2026-11-21, farming::basket). A radish cup picks
/// 0.0079 to 0.0273 of a 300 g Radish: no whole Radish, the grams kept in the player's basket. The
/// game's own save writes them (`save_home_at`), and the game's own load puts them back on a
/// player spawned the way the game spawns one (`spawn_fresh_player`, `apply_save_to_world`),
/// within 1e-12 of an item; the next cup harvested there adds to them. Not to the bit: the save is
/// JSON, and serde_json's default float parsing can land one unit in the last place away from the
/// f64 it wrote, depending on the random harvest's value, so an exact compare failed about one run
/// in several (logged FLAKY 2026-11-21: "0.010632411080340852 saved" against the kept value one
/// unit away; harmless in play, 1e-17 of a 300 g bunch). A fresh save loaded over a player holding
/// grams empties the basket, as it empties the pack: two worlds' produce never mix.
///
/// Seen red 2026-11-21 with the save not writing the basket (`extract_world_save` without its
/// `harvest_basket` line): "the save carries the kept grams: 0 saved".
#[test]
fn the_harvest_basket_survives_a_save_and_a_load() {
    use humanity_systems::farming::basket;
    const RADISH: &str = "vegetable_radish_0";
    let data_dir = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
    let mut data = farming::fixtures::gardening::make_store();
    humanity_systems::notices::register(&mut data);
    data.insert("creative_mode", Mutex::new(true));
    let last = data.get::<farming::PlantRegistry>("plant_registry").unwrap().get("radish").unwrap().last_stage().to_string();
    let radish = || humanity_core::ecs::components::CropInstance {
        crop_def_id: "radish".into(),
        growth_stage: last.clone(),
        planted_at: 0.0,
        water_level: 1.0,
        health: 100.0,
        tower_id: None,
        tower_slot: None,
        health_seconds: 1000.0,
        growing_seconds: 1000.0,
    };
    let press = |sys: &mut FarmingSystem, world: &mut hecs::World, e: hecs::Entity| {
        *data.get::<Mutex<Option<u64>>>("harvest_request").unwrap().lock().unwrap() = Some(e.to_bits().into());
        sys.tick(world, 1.0, &data);
    };

    let mut world = hecs::World::new();
    spawn_fresh_player(&mut world, &data_dir, "homestead");
    let mut sys = FarmingSystem::new();
    let cup = world.spawn((radish(),));
    press(&mut sys, &mut world, cup);
    let kept = basket::of_player(&world).toward(RADISH);
    assert!(kept > 0.0 && kept < 1.0, "a radish cup's grams are kept, short of a Radish: {kept}");

    let root = humanity_core::test_temp::dir("harvest_basket_save");
    let path = root.join("saves").join(persistence::OFFLINE_SAVE_FILE);
    save_home_at(&path, &world, &[], &data, true, None);
    let saved = persistence::load_world(&path).expect("the save reads back");
    let close = |a: f64, b: f64| (a - b).abs() < 1e-12;
    assert!(close(saved.harvest_basket.toward(RADISH), kept), "the save carries the kept grams: {} saved, {kept} kept", saved.harvest_basket.toward(RADISH));

    // The load, onto a player as the game spawns one.
    let mut world2 = hecs::World::new();
    spawn_fresh_player(&mut world2, &data_dir, "homestead");
    apply_save_to_world(&mut world2, &saved);
    assert!(close(basket::of_player(&world2).toward(RADISH), kept), "the load puts the grams back on the player: {} against {kept}", basket::of_player(&world2).toward(RADISH));
    // And they count toward the next harvest there.
    let mut sys2 = FarmingSystem::new();
    let cup2 = world2.spawn((radish(),));
    press(&mut sys2, &mut world2, cup2);
    let after = basket::of_player(&world2).toward(RADISH);
    assert!(after > kept, "the next cup adds to the loaded grams: {after} after, {kept} loaded");

    // A fresh save (nothing kept) loaded over it empties the basket.
    let fresh = persistence::WorldSave::new_offline("fresh", "homestead");
    apply_save_to_world(&mut world2, &fresh);
    assert_eq!(basket::of_player(&world2).toward(RADISH), 0.0, "a save with nothing kept empties the basket");
}

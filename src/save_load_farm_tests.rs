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
use crate::ecs::systems::System;
use crate::systems::farming::{self, step_tests, FarmingSystem};

/// The step_tests garden (every regime crossed, no air), the game's shared farm at an hour's
/// interval, and no feeder setting: the feeder's opened bags are not saved (a reload loses at most
/// one part-used item of each kind), which these tests are not about.
fn farm_home() -> (crate::hot_reload::data_store::DataStore, hecs::World, farming::Shared) {
    let (mut data, world) = step_tests::garden(false);
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

fn clock_now(data: &crate::hot_reload::data_store::DataStore) -> f64 {
    data.get::<Mutex<crate::systems::time::GameTime>>("game_time").unwrap().lock().unwrap().elapsed_seconds
}

fn set_clock(data: &crate::hot_reload::data_store::DataStore, t: f64) {
    data.get::<Mutex<crate::systems::time::GameTime>>("game_time").unwrap().lock().unwrap().set_elapsed(t);
}

fn tick(data: &crate::hot_reload::data_store::DataStore, world: &mut hecs::World, shared: &farming::Shared, dt: f32) {
    set_clock(data, clock_now(data) + f64::from(dt));
    shared.lock().unwrap().tick(world, dt, data);
}

/// How two ends differ by more than `tol` (relative, or absolute for a value under 1): every stage
/// must be the same.
fn apart(a: &step_tests::Ended, b: &step_tests::Ended, tol: f64) -> Vec<String> {
    let (stages, worst) = step_tests::compare(a, b);
    let mut out = stages;
    out.extend(worst.into_iter().filter(|(_, w)| w.abs > tol && w.rel > tol).map(|(k, w)| format!("{k}: {} ({} relative) at {}", w.abs, w.rel, w.at)));
    out
}

/// No difference at all: every stage the same and every quantity to the bit.
fn same(a: &step_tests::Ended, b: &step_tests::Ended) -> Vec<String> {
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

    let before = step_tests::ended_by_place(&world);
    let root = crate::test_temp::dir("farm_owed_save");
    let path = root.join("saves").join(persistence::OFFLINE_SAVE_FILE);
    save_home_at(&path, &world, &[], &data, true, None);
    let moved = same(&before, &step_tests::ended_by_place(&world));
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
    let differ = apart(&step_tests::ended_by_place(&world), &step_tests::ended_by_place(&world2), 1e-12);
    assert!(differ.is_empty(), "the loaded garden's first frame is not the saved one brought to now: {differ:?}");
}

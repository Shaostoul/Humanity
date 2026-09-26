//! Soil pH through the real FarmingSystem tick (2026-09-26, gardening
//! depth: soil pH). The model's pieces live in soil_ph.rs; these drive whole
//! ticks, so they prove the pieces are wired into fertilizing, the feeder,
//! the health cap, the notices, the request channel, the backpack and the
//! save. Each was seen red first; the doc comment on each says what was
//! broken to see it.

use std::collections::HashMap;
use std::sync::Mutex;

use super::gardening_tests::make_store;
use super::soil_ph::{self, SoilPhData, NITRIFICATION};
use super::*;
use crate::ecs::components::{Controllable, CropInstance, CropSoil, Irrigator, Npk, SoilMemory, UnitPh};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::inventory::{Inventory, ItemRegistry};

/// Game seconds in a garden day at 1x (farming's SECONDS_PER_DAY).
const DAY_S: f32 = 1200.0;

fn shipped() -> SoilPhData {
    SoilPhData::parse(soil_ph::SOIL_PH_RON).expect("the shipped soil_ph.ron parses")
}

/// A store with the channels pH uses, pests off (so only pH moves health),
/// crops at 100x growth, soil pH `on` or off.
fn store(on: bool) -> DataStore {
    let mut data = make_store();
    data.insert("crop_growth_speed", Mutex::new(100.0_f32));
    data.insert("garden_pest_severity", Mutex::new(0.0_f32));
    data.insert("player_notices", Mutex::new(Vec::<String>::new()));
    soil_ph::register(&mut data);
    *data.get::<Mutex<bool>>("garden_soil_ph_on").unwrap().lock().unwrap() = on;
    data
}

fn set_speed(data: &DataStore, speed: f32) {
    *data.get::<Mutex<f32>>("crop_growth_speed").unwrap().lock().unwrap() = speed;
}

/// A well-watered crop of `plant` in unit `slot` of `area`, at its first
/// stage, planted at game second 0.
fn crop(data: &DataStore, plant: &str, area: &str, slot: u32) -> CropInstance {
    let stage = data.get::<PlantRegistry>("plant_registry").unwrap().get(plant).unwrap().first_stage().to_string();
    CropInstance {
        crop_def_id: plant.to_string(),
        growth_stage: stage,
        planted_at: 0.0,
        water_level: 1.0,
        health: 100.0,
        tower_id: Some(area.to_string()),
        tower_slot: Some(slot),
        health_seconds: 0.0,
        growing_seconds: 0.0,
    }
}

/// So much of every nutrient that no crop runs short (only pH caps it).
fn rich() -> CropSoil {
    CropSoil { store: Npk::new(1e4, 1e4, 1e4), uptake: 0.0 }
}

/// Run `ticks` ticks of `dt` seconds.
fn run(sys: &mut FarmingSystem, world: &mut hecs::World, data: &DataStore, ticks: usize, dt: f32) {
    for _ in 0..ticks {
        sys.tick(world, dt, data);
    }
}

/// Run `days` garden days at 100x growth, one garden day a tick.
fn run_days(sys: &mut FarmingSystem, world: &mut hecs::World, data: &DataStore, days: usize) {
    run(sys, world, data, days, DAY_S / 100.0);
}

fn memory(world: &hecs::World) -> SoilMemory {
    world.query::<&SoilMemory>().iter().next().map(|(_, m)| m.clone()).unwrap_or_default()
}

fn unit(world: &hecs::World, area: &str, slot: u32) -> Option<UnitPh> {
    memory(world).ph.get(area).and_then(|m| m.get(&slot)).cloned()
}

/// Give unit `slot` of `area` a remembered pH (and pending pools).
fn set_unit(world: &mut hecs::World, area: &str, slot: u32, ph: f64, pending: &[(&str, f64)]) {
    let e = soil::soil_memory_entity(world);
    let mut m = world.get::<&mut SoilMemory>(e).unwrap();
    m.ph.entry(area.to_string()).or_default().insert(
        slot,
        UnitPh { ph, pending: pending.iter().map(|(k, v)| (k.to_string(), *v)).collect() },
    );
}

fn notices(data: &DataStore) -> Vec<String> {
    std::mem::take(&mut *data.get::<Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap())
}

fn request(data: &DataStore, area: &str, amendment: &str) {
    *data.get::<Mutex<Option<(String, String)>>>("soil_ph_request").unwrap().lock().unwrap() =
        Some((area.to_string(), amendment.to_string()));
}

fn fertilize(data: &DataStore, e: hecs::Entity) {
    *data.get::<Mutex<Option<u64>>>("fertilize_crop_request").unwrap().lock().unwrap() = Some(e.to_bits().into());
}

fn health_of(world: &hecs::World, e: hecs::Entity) -> f32 {
    world.get::<&CropInstance>(e).unwrap().health
}

/// The season N a crop draws, worked out the way the tick does.
fn need_n(data: &DataStore, plant: &str) -> f64 {
    let plants = data.get::<PlantRegistry>("plant_registry");
    let items = data.get::<ItemRegistry>("item_registry");
    let nd = soil::NutrientData::parse(soil::NUTRIENTS_RON).unwrap();
    let scale = plants.and_then(|r| nd.scale_for(r));
    // One plant: these tests publish no plot areas, so the tick counts one.
    crop_season_need(plant, 1, plants, items, scale).n
}

/// The pH one gram of CaCO3 equivalent moves the unit of a `plant` crop.
fn ph_per_g(data: &DataStore, ph: &SoilPhData, plant: &str) -> f64 {
    let def = data.get::<PlantRegistry>("plant_registry").unwrap().get(plant).cloned();
    let soil = ph.soil_for("bed_1").expect("beds are soil");
    soil_ph::ph_shift(&soil, soil_ph::unit_area_m2(ph, None, def.as_ref(), need_n(data, plant)), 1.0)
}

/// The cited numbers the tests below lean on, pinned to their sources (the
/// data file carries the quotes): stored urine acidifies as urea, 1.8 g of
/// CaCO3 per g of N (UNL G1503 Table 1); a garden loam's buffer is 381.1 g
/// of CaCO3 per m2 per pH unit (UC Vossen Table 1, loam 5.5 to 6.5); a unit's
/// area is its crop's N removal over the anchor tomato's 16.8 g per m2
/// (CDFA); a bed starts at 6.5 (OSU EC 1560); a tower is held at 6.0 (UF
/// HS796). Seen red by setting the loam buffer to 269 (UC's 4.5 to 5.5 row).
#[test]
fn the_cited_numbers_are_the_shipped_ones() {
    let ph = shipped();
    assert_eq!(ph.acidity_per_g_n("urine_stored_0"), 1.8);
    assert_eq!(ph.acidity_per_g_n("fertilizer_0"), 0.0, "compost: the low end of Purdue's 0 to 10+");
    let soil = ph.soil_for("bed_1").unwrap();
    assert_eq!((soil.start_ph, soil.buffer_g_caco3_per_m2), (6.5, 381.1));
    assert_eq!(ph.reference_n_removal_g_per_m2, 16.8);
    assert_eq!(ph.medium_for("ntower_3").and_then(|m| m.held_ph), Some(6.0));
    let lime = ph.amendment("lime").unwrap();
    let sulfur = ph.amendment("sulfur").unwrap();
    assert!((lime.caco3_per_g() - 1.0).abs() < 1e-12 && lime.raises());
    assert!((sulfur.caco3_per_g() + 3.12 * 0.9).abs() < 1e-12 && !sulfur.raises());
    assert_eq!((lime.half_life_days, sulfur.half_life_days, ph.nitrification_half_life_days), (30.0, 60.0, 14.0));
    assert_eq!(sulfur.max_g_per_m2, Some(97.6), "OSU EC 1560 Table 2, 20 lb per 1,000 sq ft");
    assert_eq!((ph.loss_per_ph_unit, ph.health_floor), (0.30, 20.0));
}

/// A person-day of stored urine on a tomato in a bed acidifies its unit by
/// exactly its 10.9 g of N x 1.8 g of CaCO3 per g, over the unit's buffer
/// (381.1 g per m2 x the tomato unit's 1.5 / 16.8 m2): 0.577 of a pH unit.
/// None of it arrives at once: half has reached the pH after the 14-day
/// nitrification half-life, all of it after ten. Seen red by removing the
/// `soil_ph::add_n_in` call from the Fertilize path in farming/mod.rs (the
/// unit then had no pH record at all).
#[test]
fn a_unit_fed_nitrogen_drifts_acidic_at_the_cited_rate() {
    let data = store(true);
    let ph = shipped();
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    let mut inv = Inventory::new(16);
    inv.add_item("urine_stored_0", 1, 20);
    let player = world.spawn((inv, Controllable));
    let tomato = world.spawn((crop(&data, "tomato", "bed_1", 0), rich()));

    fertilize(&data, tomato);
    sys.tick(&mut world, 0.001, &data);
    assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("urine_stored_0"), 0, "the urine went on");
    let n = soil::NutrientData::parse(soil::NUTRIENTS_RON)
        .unwrap()
        .fertilizer("urine_stored_0")
        .unwrap()
        .available_per_item(1.5)
        .n;
    assert!((n - 10.905).abs() < 1e-9, "a person-day's N: {n}");
    let expect = -n * 1.8 * ph_per_g(&data, &ph, "tomato");
    assert!((expect + 10.905 * 1.8 / (381.1 * need_n(&data, "tomato") / 16.8)).abs() < 1e-9);
    let u = unit(&world, "bed_1", 0).expect("the unit now has a pH record");
    assert!((u.ph - 6.5).abs() < 1e-4, "nothing arrives at once: {}", u.ph);
    let pending = u.pending[NITRIFICATION];
    assert!((u.ph - 6.5 + pending - expect).abs() < 1e-9, "all of it is on its way: {pending} of {expect}");

    run_days(&mut sys, &mut world, &data, 14);
    let half = unit(&world, "bed_1", 0).unwrap().ph;
    assert!((half - (6.5 + expect / 2.0)).abs() < 2e-3, "one half-life: half of it, {half} vs {}", 6.5 + expect / 2.0);
    run_days(&mut sys, &mut world, &data, 126);
    let all = unit(&world, "bed_1", 0).unwrap().ph;
    assert!((all - (6.5 + expect)).abs() < 1e-3, "ten half-lives: all of it, {all} vs {}", 6.5 + expect);
}

/// The feeder's urine acidifies too, in proportion to the N it puts in: the
/// unit's whole pH change, arrived and still nitrifying, is exactly the N
/// the feeder gave (what the unit gained plus what the crop drew) x 1.8 over
/// the unit's buffer. Seen red by removing the `soil_ph::add_n` call from
/// the feeder in farming/mod.rs (the fed unit had no pH record at all).
#[test]
fn the_feeders_urine_acidifies_the_unit_in_proportion_to_its_nitrogen() {
    let mut data = store(true);
    let ph = shipped();
    let mut stock = HashMap::new();
    stock.insert("urine_stored_0".to_string(), 20u32);
    data.insert("home_stock", Mutex::new(stock));
    let mut nut = HashMap::new();
    nut.insert("bed_1".to_string(), 1.0_f32);
    data.insert("garden_nutrient", Mutex::new(nut));
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    let tomato = world.spawn((crop(&data, "tomato", "bed_1", 0), CropSoil { store: Npk::ZERO, uptake: 0.0 }));
    run_days(&mut sys, &mut world, &data, 20);

    let s = world.get::<&CropSoil>(tomato).unwrap().clone();
    let need = need_n(&data, "tomato");
    let fed_n = s.store.n + need * f64::from(s.uptake);
    assert!(fed_n > 0.1, "the feeder gave N: {fed_n}");
    let u = unit(&world, "bed_1", 0).expect("the fed unit has a pH record");
    let moved = u.ph - 6.5 + u.pending.values().sum::<f64>();
    let expect = -fed_n * 1.8 * ph_per_g(&data, &ph, "tomato");
    assert!((moved - expect).abs() < 1e-6 * expect.abs().max(1e-3), "moved {moved} vs {expect}");
    assert!(u.ph < 6.5, "and some of it has already arrived: {}", u.ph);
}

/// Lime brings a bed toward the middle of its crop's window and sulfur
/// brings another down to it, each only as it reacts: nothing at once, half
/// of the lime after its 30-day half-life while the slower sulfur (60 days)
/// has done 1 - 0.5^0.5 = 29% of its work, and both at the lettuce's 6.5
/// after 600 days. One bag of each is used from the backpack, and a second
/// press asks for no more, counting what is still reacting. Seen red by
/// making `step_unit` react every pool at once (the limed bed read 6.5
/// straight after the request).
#[test]
fn lime_raises_the_ph_and_sulfur_lowers_it_after_its_delay() {
    let data = store(true);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    let mut inv = Inventory::new(16);
    inv.add_item("garden_lime_0", 1, 10);
    inv.add_item("garden_sulfur_0", 1, 10);
    let player = world.spawn((inv, Controllable));
    for slot in 0..2 {
        world.spawn((crop(&data, "lettuce", "bed_1", slot), rich()));
        set_unit(&mut world, "bed_1", slot, 5.5, &[]);
    }
    world.spawn((crop(&data, "lettuce", "bed_2", 0), rich()));
    set_unit(&mut world, "bed_2", 0, 7.1, &[]);

    request(&data, "bed_1", "lime");
    sys.tick(&mut world, 0.001, &data);
    request(&data, "bed_2", "sulfur");
    sys.tick(&mut world, 0.001, &data);
    let said = notices(&data).join(" | ");
    assert!(said.contains("Lime on the soil in bed 1") && said.contains("2 units"), "{said}");
    assert!(said.contains("Sulfur on the soil in bed 2"), "{said}");
    {
        let inv = world.get::<&Inventory>(player).unwrap();
        assert_eq!((inv.count_item("garden_lime_0"), inv.count_item("garden_sulfur_0")), (0, 0), "a bag of each");
    }
    let limed = unit(&world, "bed_1", 1).unwrap();
    let sulfured = unit(&world, "bed_2", 0).unwrap();
    assert!((limed.ph - 5.5).abs() < 1e-4 && (sulfured.ph - 7.1).abs() < 1e-4, "nothing at once");
    assert!((limed.pending["lime"] - 1.0).abs() < 1e-3, "lime to 6.5: {:?}", limed.pending);
    assert!((sulfured.pending["sulfur"] + 0.6).abs() < 1e-3, "sulfur to 6.5: {:?}", sulfured.pending);

    // Pressing Lime again now asks for nothing: it is already on its way.
    request(&data, "bed_1", "lime");
    sys.tick(&mut world, 0.001, &data);
    assert!(notices(&data).join(" ").contains("already up to"), "no second dose");

    run_days(&mut sys, &mut world, &data, 30);
    let a = unit(&world, "bed_1", 0).unwrap().ph;
    let b = unit(&world, "bed_2", 0).unwrap().ph;
    assert!((a - 6.0).abs() < 5e-3, "lime, one half-life: half way, {a}");
    let sulfur_part = 1.0 - 0.5f64.powf(0.5);
    assert!((b - (7.1 - 0.6 * sulfur_part)).abs() < 5e-3, "sulfur, half its half-life: {b}");
    run_days(&mut sys, &mut world, &data, 570);
    let a = unit(&world, "bed_1", 0).unwrap().ph;
    let b = unit(&world, "bed_2", 0).unwrap().ph;
    assert!((a - 6.5).abs() < 5e-3 && (b - 6.5).abs() < 5e-3, "both at the lettuce's middle: {a}, {b}");
}

/// Sulfur is given a little at a time: a bed far above its crop's window
/// gets at most OSU EC 1560's 97.6 g of sulfur per m2, 0.80 of a pH unit in
/// a loam, and is told to give it again later; and a tower, whose solution
/// is held, is refused. Seen red by dropping the `max_g_per_m2` cap in
/// `handle_request` (the bed got the whole 1.6 units).
#[test]
fn sulfur_is_capped_per_application_and_towers_are_refused() {
    let mut data = store(true);
    set_speed(&data, 1.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    world.spawn((crop(&data, "lettuce", "bed_1", 0), rich()));
    set_unit(&mut world, "bed_1", 0, 8.1, &[]);
    world.spawn((crop(&data, "lettuce", "ntower_2", 0), rich()));
    world.spawn((Inventory::new(16), Controllable));
    data.insert("creative_mode", Mutex::new(true));

    request(&data, "bed_1", "sulfur");
    sys.tick(&mut world, 0.001, &data);
    let said = notices(&data).join(" ");
    let cap = -97.6 * 3.12 / 381.1;
    let got = unit(&world, "bed_1", 0).unwrap().pending["sulfur"];
    assert!((got - cap).abs() < 1e-6, "the most one application gives: {got} vs {cap}");
    assert!(said.contains("give it again"), "{said}");

    request(&data, "ntower_2", "lime");
    sys.tick(&mut world, 0.001, &data);
    assert!(notices(&data).join(" ").contains("held at 6.0"), "a tower's pH is its dosing's job");
    assert!(unit(&world, "ntower_2", 0).is_none(), "and nothing is recorded for it");
}

/// Outside its window a crop's health is capped by 30% per pH unit (the
/// NRCS table's median): a lettuce (6.0 to 7.0) at pH 5.0 settles at 70, one
/// at 8.3 at 61, and one at 6.5 keeps 100. A tower's lettuce is not capped
/// (its solution is held). The player is told once per bed, with the fix,
/// and not again. Seen red by leaving `.min(ph_ceiling)` off the crop's
/// ceiling in farming/mod.rs (every lettuce kept 100).
#[test]
fn a_crop_outside_its_window_is_capped_and_one_inside_is_not() {
    let data = store(true);
    set_speed(&data, 1.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    let acid = world.spawn((crop(&data, "lettuce", "bed_1", 0), rich()));
    set_unit(&mut world, "bed_1", 0, 5.0, &[]);
    let fine = world.spawn((crop(&data, "lettuce", "bed_2", 0), rich()));
    let alkaline = world.spawn((crop(&data, "lettuce", "bed_3", 0), rich()));
    set_unit(&mut world, "bed_3", 0, 8.3, &[]);
    let tower = world.spawn((crop(&data, "lettuce", "ntower_1", 0), rich()));

    run(&mut sys, &mut world, &data, 450, 1.0);
    assert!((health_of(&world, acid) - 70.0).abs() < 0.2, "1.0 below: {}", health_of(&world, acid));
    assert!((health_of(&world, alkaline) - 61.0).abs() < 0.2, "1.3 above: {}", health_of(&world, alkaline));
    assert_eq!(health_of(&world, fine), 100.0, "inside the window");
    assert_eq!(health_of(&world, tower), 100.0, "a held solution");
    let said = notices(&data);
    assert_eq!(said.len(), 2, "one per bed out of its window: {said:?}");
    assert!(said.iter().any(|s| s.contains("bed 1 is too acid for its lettuce") && s.contains("Use Lime in the Garden panel")), "{said:?}");
    assert!(said.iter().any(|s| s.contains("bed 3 is too alkaline") && s.contains("Use Sulfur in the Garden panel")), "{said:?}");
    run(&mut sys, &mut world, &data, 50, 1.0);
    assert!(notices(&data).is_empty(), "said once");
    // The cap never goes below the floor, and a crop with no window has none.
    let ph = shipped();
    assert_eq!(soil_ph::health_ceiling(&ph, 3.0, 6.0, 7.0), 20.0);
    assert_eq!(soil_ph::health_ceiling(&ph, 3.0, 0.0, 0.0), 100.0);
}

/// Off (Settings "Soil pH: Off") changes nothing: an acid bed's lettuce
/// keeps full health, urine adds no acidity, a pending lime pool stays where
/// it is, a Lime request takes no bag and does nothing, and nothing is said.
/// Seen red by making `soil_ph::is_on` ignore the channel (the Lime request
/// then took a bag).
#[test]
fn off_mode_has_no_effect() {
    let data = store(false);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    let mut inv = Inventory::new(16);
    inv.add_item("urine_stored_0", 1, 20);
    inv.add_item("garden_lime_0", 1, 10);
    let player = world.spawn((inv, Controllable));
    let acid = world.spawn((crop(&data, "lettuce", "bed_1", 0), rich()));
    set_unit(&mut world, "bed_1", 0, 5.0, &[("lime", 1.0)]);
    let fed = world.spawn((crop(&data, "tomato", "bed_2", 0), rich()));

    fertilize(&data, fed);
    sys.tick(&mut world, 0.001, &data);
    request(&data, "bed_1", "lime");
    sys.tick(&mut world, 0.001, &data);
    run_days(&mut sys, &mut world, &data, 40);

    let inv = world.get::<&Inventory>(player).unwrap();
    assert_eq!(inv.count_item("urine_stored_0"), 0, "fertilizing still works");
    assert_eq!(inv.count_item("garden_lime_0"), 1, "no lime taken");
    assert!(unit(&world, "bed_2", 0).is_none(), "no acidity recorded");
    assert_eq!(unit(&world, "bed_1", 0).unwrap(), UnitPh { ph: 5.0, pending: [("lime".to_string(), 1.0)].into() });
    assert_eq!(health_of(&world, acid), 100.0, "not capped");
    assert!(notices(&data).iter().all(|s| !s.contains("pH")), "nothing said about pH");
}

/// A unit's pH and what is still reacting come back after a save; a save
/// from before pH existed loads with none, and its units read the bed's
/// starting 6.5. Seen red by marking `SoilMemory::ph` `#[serde(skip)]` (the
/// record came back empty).
#[cfg(feature = "native")]
#[test]
fn ph_survives_a_save_and_old_saves_read_the_default() {
    use crate::persistence::WorldSave;
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

/// The reactions are first-order and solved exactly, so a stretch of time
/// gives the same pH however it is sliced into ticks (a frame, a clock jump),
/// and one half-life does exactly half. Seen red by stepping them with the
/// linear rate `days x ln 2 / half-life` (the sliced and whole runs parted).
#[test]
fn reactions_are_exact_however_the_time_is_sliced() {
    let ph = shipped();
    let start = UnitPh { ph: 6.0, pending: [("lime".to_string(), 0.8), ("sulfur".to_string(), -0.3)].into() };
    let mut once = start.clone();
    soil_ph::step_unit(&mut once, &ph, 90.0);
    let mut sliced = start.clone();
    for _ in 0..9000 {
        soil_ph::step_unit(&mut sliced, &ph, 0.01);
    }
    assert!((once.ph - sliced.ph).abs() < 1e-9, "{} vs {}", once.ph, sliced.ph);
    let mut half = UnitPh { ph: 6.0, pending: [("lime".to_string(), 0.8)].into() };
    soil_ph::step_unit(&mut half, &ph, 30.0);
    assert!((half.ph - 6.4).abs() < 1e-12 && (half.pending["lime"] - 0.4).abs() < 1e-12, "{half:?}");
    let mut over = UnitPh { ph: 8.2, pending: [("lime".to_string(), 5.0)].into() };
    soil_ph::step_unit(&mut over, &ph, 1000.0);
    assert_eq!(over.ph, 8.3, "lime cannot take a soil past calcium carbonate's 8.3");
}

/// The shipped data, as data: every amendment names a real item that the
/// vendor and the Farming Elder sell, every fertilizer with an acidity is a
/// fertilizer the nutrient model applies, every yield-table crop is a
/// plants.csv crop, and `loss_per_ph_unit` IS the median loss per pH unit of
/// the cited NRCS table's points outside their crops' plants.csv windows.
/// Seen red two ways: "garden_lime_0" misspelt "garden_lim_0" in soil_ph.ron
/// (the items check named it), and loss_per_ph_unit set to 0.25 (the median
/// check gave 0.30).
#[test]
fn the_shipped_ph_data_names_real_items_and_matches_its_table() {
    let ph = shipped();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let items = ItemRegistry::from_csv(&std::fs::read(root.join("data/items.csv")).unwrap()).unwrap();
    let plants = PlantRegistry::from_csv(&std::fs::read(root.join("data/plants.csv")).unwrap()).unwrap();
    let goods = std::fs::read_to_string(root.join("data/trade_goods.ron")).unwrap();
    let npcs = std::fs::read_to_string(root.join("data/npcs.ron")).unwrap();
    let nutrients = soil::NutrientData::parse(soil::NUTRIENTS_RON).unwrap();
    assert_eq!(ph.amendments.len(), 2, "lime and sulfur");
    for a in &ph.amendments {
        assert!(items.items.contains_key(&a.item), "{}: {} is not in items.csv", a.id, a.item);
        assert!(goods.contains(&format!("id: \"{}\"", a.item)), "{}: the vendor does not sell {}", a.id, a.item);
        assert!(npcs.contains(&format!("(\"{}\",", a.item)), "{}: no NPC shop sells {}", a.id, a.item);
        assert!(a.half_life_days > 0.0 && a.purity > 0.0 && a.purity <= 1.0, "{}", a.id);
    }
    for f in &ph.fertilizer_acidity {
        assert!(nutrients.fertilizer(&f.item).is_some(), "{} is not a nutrients.ron fertilizer", f.item);
    }
    assert!(ph.medium_for(&ph.default_medium).map_or(false, |m| m.soil.is_some()), "the default medium is a soil");

    // The median, recomputed from the table and plants.csv windows.
    let mut losses: Vec<f64> = Vec::new();
    for row in &ph.yield_table {
        let d = plants.get(&row.plant).unwrap_or_else(|| panic!("{} is not in plants.csv", row.plant));
        let (lo, hi) = (f64::from(d.ph_min), f64::from(d.ph_max));
        assert_eq!(row.relative_yield.len(), ph.yield_table_ph.len(), "{}", row.plant);
        for (p, y) in ph.yield_table_ph.iter().zip(&row.relative_yield) {
            let out = soil_ph::outside(*p, lo, hi);
            if out > 0.0 {
                losses.push((100.0 - y) / 100.0 / out);
            }
        }
    }
    losses.sort_by(|a, b| a.total_cmp(b));
    assert_eq!(losses.len(), 17, "the table's points outside their windows");
    let median = losses[losses.len() / 2];
    assert!((median - ph.loss_per_ph_unit).abs() < 0.005, "median {median} vs shipped {}", ph.loss_per_ph_unit);
}

/// Every grow machine in both shipped homes lands in the medium its type
/// grows in, by its instance id (the showcase tags crops that way) and by
/// its type (a bed group's crops carry the type), and every tower design in
/// data/towers/aeroponic_configs.ron is a tower: the sim sees only the tag,
/// so a new tower prefix must not silently read as soil. Seen red by
/// dropping "ctower_" from the aeroponic prefixes (the commons towers read
/// as soil).
#[test]
fn every_shipped_grow_area_lands_in_its_medium() {
    let ph = shipped();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let expected = |machine: &str| -> Option<&'static str> {
        if machine.starts_with("aeroponic_tower") {
            Some("aeroponic")
        } else if machine == "mushroom_rack" {
            Some("mushroom")
        } else if machine.ends_with("_bed") || machine.ends_with("_field") || machine.ends_with("_tray") {
            Some("soil")
        } else {
            None
        }
    };
    let mut checked = 0;
    for file in ["data/machines/home.ron", "data/machines/home_solo.ron"] {
        let home = crate::machines::MachineHome::load(&root.join(file)).unwrap_or_else(|| panic!("{file} loads"));
        for inst in home.all_instances() {
            let Some(want) = expected(&inst.machine) else { continue };
            for tag in [inst.id.as_str(), inst.machine.as_str()] {
                let got = ph.medium_for(tag).map(|m| m.id.as_str());
                assert_eq!(got, Some(want), "{file}: {tag} ({}) grows in {want}", inst.machine);
            }
            checked += 1;
        }
    }
    assert!(checked > 40, "grow machines checked: {checked}");
    #[derive(serde::Deserialize)]
    struct Towers {
        towers: Vec<Tower>,
    }
    #[derive(serde::Deserialize)]
    struct Tower {
        id: String,
    }
    let towers: Towers =
        ron::from_str(&std::fs::read_to_string(root.join("data/towers/aeroponic_configs.ron")).unwrap()).unwrap();
    assert!(!towers.towers.is_empty());
    for t in &towers.towers {
        assert_eq!(ph.medium_for(&t.id).map(|m| m.id.as_str()), Some("aeroponic"), "tower design {}", t.id);
    }
    for tag in ["", "bed_1", "grain_field_1", "courtbed_2"] {
        assert_eq!(ph.medium_for(tag).map(|m| m.id.as_str()), Some("soil"), "{tag:?}");
    }
}

/// A plot of a placed bed buffers over its real floor (2026-09-26, the
/// engine's "grow_plot_area_m2"), not over the area inferred from its crop's
/// nitrogen: the same acid moves a 1 m2 plot twice as far as a 2 m2 one, and
/// the map is what `plot_area` reads. Seen red by making `unit_area_m2`
/// ignore `plot_m2` (both plots then moved alike, by the inferred area).
#[test]
fn a_placed_plot_buffers_over_its_real_floor_area() {
    let ph = shipped();
    let soil = ph.soil_for("graintray_0").expect("trays are soil");
    let one = soil_ph::ph_shift(&soil, soil_ph::unit_area_m2(&ph, Some(1.0), None, 50.0), -381.1);
    let two = soil_ph::ph_shift(&soil, soil_ph::unit_area_m2(&ph, Some(2.0), None, 50.0), -381.1);
    assert!((one + 1.0).abs() < 1e-9, "a 1 m2 plot moves a full unit: {one}");
    assert!((two + 0.5).abs() < 1e-9, "a 2 m2 plot moves half: {two}");
    let mut store = DataStore::new();
    let mut m = std::collections::HashMap::new();
    m.insert("graintray_0".to_string(), 1.0_f32);
    store.insert("grow_plot_area_m2", m);
    assert_eq!(soil_ph::plot_area(&store, "graintray_0"), Some(1.0));
    assert_eq!(soil_ph::plot_area(&store, "ntower_3"), None, "a tower has no plot floor");
}

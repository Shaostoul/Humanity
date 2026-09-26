//! Weeds through the real FarmingSystem tick (2026-09-26, gardening depth).
//! The model's pieces are unit-tested in weeds.rs; these drive whole ticks, so
//! they prove the pieces are wired into the growth loop, the health cap, the
//! soil's nitrogen, the control channel, the backpack and the save. Each was
//! seen red first; the doc comment on each says what was broken to see it.

use super::gardening_tests::make_store;
use super::weeds::{self, WeedData};
use super::*;
use crate::ecs::components::{AreaWeeds, Controllable, CropInstance, CropSoil, Irrigator, Npk, SoilMemory};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::inventory::Inventory;

/// A well-watered crop of `plant` in unit `slot` of `area`, `days` garden
/// days into its season at growth `speed` (the clock stands at 0).
fn crop(data: &DataStore, plant: &str, area: &str, slot: u32, days: f64, speed: f32) -> CropInstance {
    let stage = data.get::<PlantRegistry>("plant_registry").unwrap().get(plant).unwrap().first_stage().to_string();
    CropInstance {
        crop_def_id: plant.to_string(),
        growth_stage: stage,
        planted_at: -days * SECONDS_PER_DAY / f64::from(speed),
        water_level: 1.0,
        health: 100.0,
        tower_id: Some(area.to_string()),
        tower_slot: Some(slot),
        health_seconds: 0.0,
        growing_seconds: 0.0,
    }
}

/// A store with the channels weeds use, at `speed` growth and `severity`.
fn store(speed: f32, severity: f32) -> DataStore {
    let mut data = make_store();
    data.insert("crop_growth_speed", std::sync::Mutex::new(speed));
    data.insert("garden_pest_severity", std::sync::Mutex::new(severity));
    data.insert("player_notices", std::sync::Mutex::new(Vec::<String>::new()));
    data.insert(weeds::REQUEST_KEY, std::sync::Mutex::new(Option::<(String, String)>::None));
    data
}

fn run(sys: &mut FarmingSystem, world: &mut hecs::World, data: &DataStore, ticks: usize, dt: f32) {
    for _ in 0..ticks {
        sys.tick(world, dt, data);
    }
}

fn memory(world: &hecs::World) -> SoilMemory {
    world.query::<&SoilMemory>().iter().next().map(|(_, m)| m.clone()).unwrap_or_default()
}

fn cover(world: &hecs::World, area: &str) -> f64 {
    memory(world).weeds.get(area).map_or(0.0, |w| w.level)
}

/// Put weeds at `level` in `area` over a seed bank of `bank` (told already,
/// so no notice).
fn seed_weeds(world: &mut hecs::World, area: &str, level: f64, bank: f64) {
    let e = soil::soil_memory_entity(world);
    let mut m = world.get::<&mut SoilMemory>(e).unwrap();
    m.weeds.insert(area.to_string(), AreaWeeds { level, bank, told: true, ..Default::default() });
}

fn notices(data: &DataStore) -> Vec<String> {
    std::mem::take(&mut *data.get::<std::sync::Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap())
}

fn request(data: &DataStore, area: &str, control: &str) {
    *data.get::<std::sync::Mutex<Option<(String, String)>>>(weeds::REQUEST_KEY).unwrap().lock().unwrap() =
        Some((area.to_string(), control.to_string()));
}

fn health(world: &hecs::World, area: &str) -> f32 {
    world
        .query::<&CropInstance>()
        .iter()
        .find(|(_, c)| c.tower_id.as_deref() == Some(area))
        .map(|(_, c)| c.health)
        .unwrap()
}

/// Weeds come up from the seed bank in soil and nowhere else: over thirty
/// garden days a bed of tomatoes indoors gets weedy, an outdoor wheat field
/// (a typical field's seed bank, four times the indoor bed's) gets weedier,
/// and a tower of lettuce, which grows in a nutrient mist, never has a weed
/// entry at all. The player is told once when the bed's weeds come up. Seen
/// red by treating every grow area as soil in `Weeds::step` (the tower then
/// grew weeds).
#[test]
fn weeds_build_in_a_soil_bed_and_not_in_a_tower() {
    let data = store(100.0, 1.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    for slot in 0..4 {
        world.spawn((crop(&data, "tomato", "tomato_bed", slot, 0.0, 100.0),));
        world.spawn((crop(&data, "lettuce", "ntower_1", slot, 0.0, 100.0),));
        world.spawn((crop(&data, "wheat", "grain_field_1", slot, 0.0, 100.0),));
    }
    // 100x growth, 1 s ticks: a garden day is 12 ticks; thirty days.
    run(&mut sys, &mut world, &data, 360, 1.0);
    let (bed, field) = (cover(&world, "tomato_bed"), cover(&world, "grain_field_1"));
    assert!(bed > 0.05, "weeds came up in the bed: {bed}");
    assert!(field > bed, "and faster in the field: {field} vs {bed}");
    assert!(!memory(&world).weeds.contains_key("ntower_1"), "no soil in a tower, no weeds");
    let said = notices(&data);
    assert_eq!(said.iter().filter(|n| n.contains("Weeds are coming up in the soil in tomato bed")).count(), 1, "{said:?}");
}

/// Through the tick, full weed cover caps a tomato hardest in its critical
/// period (UMass: days 23 to 41): three beds at full cover hold a tomato 10,
/// 30 and 50 garden days into its season to about 88, 53 and 76, and a tower
/// tomato beside them keeps full health. Seen red by leaving the weed cap
/// out of the `min` that binds (every tomato stayed at 100).
#[test]
fn weeds_cap_an_out_competed_crop_hardest_in_its_critical_period() {
    let data = store(1.0, 1.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    for (area, days) in [("bed_early", 10.0), ("bed_critical", 30.0), ("bed_late", 50.0), ("ntower_1", 30.0)] {
        world.spawn((crop(&data, "tomato", area, 0, days, 1.0),));
        seed_weeds(&mut world, area, 1.0, 1.0);
    }
    // 1x growth, so the crops' ages hardly move: 600 s is half a garden day,
    // and health falls to its cap at 0.1 a second.
    run(&mut sys, &mut world, &data, 600, 1.0);
    let (early, critical, late) = (health(&world, "bed_early"), health(&world, "bed_critical"), health(&world, "bed_late"));
    assert!((critical - 53.0).abs() < 1.0, "in its critical period: {critical}");
    assert!((late - 76.5).abs() < 1.0 && (early - 88.25).abs() < 1.0, "{early} / {late}");
    assert!(critical < late && late < early);
    assert_eq!(health(&world, "ntower_1"), 100.0, "a tower has no soil for weeds");
}

/// Hoeing takes 90% of small weeds and 60% of big ones (NEVG's figure for
/// cultivation), uses the backpack's hoe, and wears it: a hoe one use from
/// worn out breaks, and with no hoe the job is refused and nothing changes.
/// Seen red by leaving out the `wear_item` call (the worn hoe survived).
#[test]
fn hoeing_removes_weeds_and_wears_the_hoe() {
    let data = store(1.0, 1.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    let mut inv = Inventory::new(16);
    inv.add_item("hoe_0", 1, 1);
    let player = world.spawn((inv, Controllable));
    for slot in 0..4 {
        world.spawn((crop(&data, "tomato", "tomato_bed", slot, 5.0, 1.0),));
    }
    seed_weeds(&mut world, "tomato_bed", 0.2, 0.25);
    request(&data, "tomato_bed", "hoe");
    sys.tick(&mut world, 0.016, &data);
    assert!((cover(&world, "tomato_bed") - 0.02).abs() < 1e-3, "small weeds: 90% gone, {}", cover(&world, "tomato_bed"));
    let wear = |w: &hecs::World| w.get::<&Inventory>(player).unwrap().slots.iter().flatten().find(|s| s.item_id == "hoe_0").map(|s| s.wear);
    assert_eq!(wear(&world), Some(1), "one use of the hoe for 1.7 m2");
    // Big weeds: a pass gets 60%.
    seed_weeds(&mut world, "tomato_bed", 0.6, 0.25);
    request(&data, "tomato_bed", "hoe");
    sys.tick(&mut world, 0.016, &data);
    assert!((cover(&world, "tomato_bed") - 0.24).abs() < 1e-3, "{}", cover(&world, "tomato_bed"));
    // A hoe one use from worn out (items.csv durability 150) breaks.
    world.get::<&mut Inventory>(player).unwrap().slots.iter_mut().flatten().for_each(|s| s.wear = 149);
    notices(&data);
    request(&data, "tomato_bed", "hoe");
    sys.tick(&mut world, 0.016, &data);
    assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("hoe_0"), 0, "the hoe wore out");
    assert!(notices(&data).iter().any(|n| n.contains("wore out")));
    // No hoe: refused, the weeds as they were.
    seed_weeds(&mut world, "tomato_bed", 0.3, 0.25);
    request(&data, "tomato_bed", "hoe");
    sys.tick(&mut world, 0.016, &data);
    assert!((cover(&world, "tomato_bed") - 0.3).abs() < 1e-3, "no hoe, no hoeing");
    assert!(notices(&data).iter().any(|n| n.contains("needs a hoe")));
}

/// A sawdust mulch spends its cited 2.441 kg a square metre (OSU: 50 lb on a
/// 10 ft x 10 ft plot) from the backpack, smothers half the weeds there, and
/// keeps new ones down: over sixty garden days a mulched bed stays nearly
/// clear while its unmulched twin fills up. It lasts its 150 garden days and
/// bark its 1,095 (Clemson: 2 to 4 years), then is gone; and without enough
/// sawdust the job is refused with nothing spent. Seen red by ignoring the
/// mulch in `step_area` (the mulched bed filled up like its twin).
#[test]
fn mulch_prevents_weeds_for_its_cited_time_and_spends_its_item() {
    let data = store(100.0, 1.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    let mut inv = Inventory::new(16);
    inv.add_item("sawdust_0", 3, 20);
    let player = world.spawn((inv, Controllable));
    for slot in 0..4 {
        world.spawn((crop(&data, "tomato", "bed_a", slot, 0.0, 100.0),));
        world.spawn((crop(&data, "tomato", "bed_b", slot, 0.0, 100.0),));
    }
    seed_weeds(&mut world, "bed_a", 0.2, 1.0);
    seed_weeds(&mut world, "bed_b", 0.2, 1.0);
    // Four tomato units of 0.418 m2 (plants.csv spacing) are 1.672 m2: 4.08 kg,
    // five 1 kg bags of sawdust. Three are not enough.
    request(&data, "bed_a", "mulch_sawdust");
    sys.tick(&mut world, 0.016, &data);
    assert!(notices(&data).iter().any(|n| n.contains("takes 5 x Sawdust; you have 3")));
    assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("sawdust_0"), 3, "nothing spent");
    world.get::<&mut Inventory>(player).unwrap().add_item("sawdust_0", 3, 20);
    request(&data, "bed_a", "mulch_sawdust");
    sys.tick(&mut world, 0.016, &data);
    assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("sawdust_0"), 1, "five bags laid");
    assert!((cover(&world, "bed_a") - 0.1).abs() < 1e-3, "half smothered: {}", cover(&world, "bed_a"));
    run(&mut sys, &mut world, &data, 720, 1.0);
    let (mulched, bare) = (cover(&world, "bed_a"), cover(&world, "bed_b"));
    assert!(bare > 0.9, "the bare bed filled up: {bare}");
    assert!(mulched < 0.4, "the mulched one did not: {mulched}");
    // It lasts its cited time, and bark far longer.
    let d = WeedData::shipped();
    let mut a = AreaWeeds { bank: 1.0, ..Default::default() };
    a.mulch.insert("mulch_sawdust".into(), d.control("mulch_sawdust").unwrap().lasts_days);
    a.mulch.insert("mulch_bark".into(), d.control("mulch_bark").unwrap().lasts_days);
    weeds::step_area(&mut a, &d, 149.0);
    assert!(a.mulch.contains_key("mulch_sawdust"), "sawdust still working at 149 days");
    weeds::step_area(&mut a, &d, 2.0);
    assert!(!a.mulch.contains_key("mulch_sawdust") && a.mulch.contains_key("mulch_bark"), "gone at 151; bark stays");
    weeds::step_area(&mut a, &d, 945.0);
    assert!(a.mulch.is_empty(), "bark gone after 1,096 days");
}

/// Sawdust ties up nitrogen as it rots (OSU: "some of the nitrogen in the soil
/// would be used for sawdust de-composition after mulching"): 20 g of N per
/// kg, spread over its 150 garden days. Over ten garden days a mulched tomato
/// unit's soil gives up 2.441 x 20 / 150 x 0.418 m2 x 10 = 1.36 g more N than
/// its unmulched twin's, and the same grams wait in the unit's slow organic
/// pool, to come back as the sawdust rots. Seen red by dropping the draw in
/// `Weeds::crop_tick` (the two units ended level).
#[test]
fn sawdust_mulch_ties_up_nitrogen() {
    let mut data = store(100.0, 1.0);
    data.insert("creative_mode", std::sync::Mutex::new(true));
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    let rich = || CropSoil { store: Npk::new(100.0, 100.0, 100.0), uptake: 0.0 };
    let a = world.spawn((crop(&data, "tomato", "bed_a", 0, 0.0, 100.0), rich()));
    let b = world.spawn((crop(&data, "tomato", "bed_b", 0, 0.0, 100.0), rich()));
    request(&data, "bed_a", "mulch_sawdust");
    run(&mut sys, &mut world, &data, 120, 1.0);
    let n = |e: hecs::Entity| world.get::<&CropSoil>(e).unwrap().store.n;
    let tied = n(b) - n(a);
    let want = 2.441 * 20.0 / 150.0 * 0.418 * 10.0;
    assert!((tied - want).abs() < 0.03, "tied up {tied} g, want {want}");
    let banked: f64 = memory(&world).organic.get("bed_a").and_then(|m| m.get(&0)).map_or(0.0, |p| p.iter().map(|c| c.n).sum());
    assert!((banked - tied).abs() < 0.03, "banked {banked} g as slow organic N");
}

/// Off (the Settings severity at 0) turns weeds off: nothing comes up, full
/// cover caps nothing, and a hoe request neither hoes nor wears the hoe.
/// Seen red by stepping the weeds whatever the severity in `Weeds::step`
/// (the bed grew weeds with the switch off).
#[test]
fn off_mode_turns_weeds_off() {
    let data = store(100.0, 0.0);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    world.spawn((Irrigator,));
    let mut inv = Inventory::new(16);
    inv.add_item("hoe_0", 1, 1);
    let player = world.spawn((inv, Controllable));
    world.spawn((crop(&data, "tomato", "tomato_bed", 0, 0.0, 100.0),));
    world.spawn((crop(&data, "tomato", "bed_weedy", 0, 30.0, 100.0),));
    seed_weeds(&mut world, "bed_weedy", 1.0, 1.0);
    run(&mut sys, &mut world, &data, 360, 1.0);
    assert!(!memory(&world).weeds.contains_key("tomato_bed"), "no weeds come up with weeds off");
    assert_eq!(cover(&world, "bed_weedy"), 1.0, "and nothing steps");
    assert_eq!(health(&world, "bed_weedy"), 100.0, "full cover caps nothing");
    request(&data, "bed_weedy", "hoe");
    sys.tick(&mut world, 0.016, &data);
    assert_eq!(cover(&world, "bed_weedy"), 1.0, "the hoe does nothing");
    let worn = world.get::<&Inventory>(player).unwrap().slots.iter().flatten().any(|s| s.item_id == "hoe_0" && s.wear > 0);
    assert!(!worn, "and is not worn");
}

/// The weeds are saved: they live on `SoilMemory`, which the world save
/// writes whole, so a round trip keeps an area's cover, seed bank, notice and
/// mulch days; and a save from before weeds existed loads with none. Seen red
/// by marking `SoilMemory::weeds` `#[serde(skip)]` (the bed came back empty).
/// Native only: the world save lives in the desktop build (save_load and
/// persistence are native-gated), as in soil_ph_tests.
#[cfg(feature = "native")]
#[test]
fn weeds_survive_a_save_and_old_saves_load_with_none() {
    use crate::save_load::{apply_save_to_world, extract_world_save};
    let mut world = hecs::World::new();
    let mut memory_in = SoilMemory::default();
    let mut w = AreaWeeds { level: 0.37, bank: 1.4, told: true, ..Default::default() };
    w.mulch.insert("mulch_bark".into(), 812.5);
    memory_in.weeds.insert("tomato_bed".into(), w.clone());
    world.spawn((memory_in,));
    let text = serde_json::to_string(&extract_world_save(&world)).unwrap();
    let back: crate::persistence::WorldSave = serde_json::from_str(&text).unwrap();
    let mut fresh = hecs::World::new();
    apply_save_to_world(&mut fresh, &back);
    assert_eq!(memory(&fresh).weeds.get("tomato_bed"), Some(&w), "the bed's weeds came back");
    let mut old: serde_json::Value = serde_json::from_str(&text).unwrap();
    old["soil_memory"].as_object_mut().unwrap().remove("weeds");
    let back: crate::persistence::WorldSave = serde_json::from_value(old).unwrap();
    let mut older = hecs::World::new();
    apply_save_to_world(&mut older, &back);
    assert!(memory(&older).weeds.is_empty(), "an old save loads with no weeds");
}

/// The shipped data parses, names real plants and real items (the hoe the
/// player starts with, the sawmill's sawdust and bark), lists the hoe before
/// the mulches (the order growers are taught), and carries its sources'
/// arithmetic: the tomato and wheat windows, FAO's first third, the median of
/// WSSA's table, OSU's 50 lb of sawdust on 100 sq ft and 1 lb of N per 50 lb,
/// Clemson's 2 to 4 years of bark, and no crop losing more than 0.8. Seen red
/// by misspelling "sawdust_0" as "sawdust0" in weeds.ron.
#[test]
fn the_shipped_weed_data_parses_and_names_real_items() {
    let d = WeedData::shipped();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let plants = PlantRegistry::from_csv(&std::fs::read(root.join("data/plants.csv")).unwrap()).unwrap();
    let items = crate::systems::inventory::ItemRegistry::from_csv(&std::fs::read(root.join("data/items.csv")).unwrap()).unwrap();
    for row in &d.crops {
        for p in &row.plants {
            assert!(plants.get(p).is_some(), "'{p}' is not in plants.csv");
            assert!(d.max_loss(p) > 0.0 && d.max_loss(p) <= 0.8, "{p}: no wipe-outs");
        }
        if let (Some(s), Some(e)) = (row.start_days, row.end_days) {
            assert!(0.0 <= s && s < e, "{:?}", row.plants);
        }
    }
    for c in &d.controls {
        for id in [&c.tool, &c.item] {
            if !id.is_empty() {
                assert!(items.items.contains_key(id.as_str()), "{}: '{id}' is not in items.csv", c.id);
            }
        }
    }
    let kinds: Vec<weeds::WeedControlKind> = d.controls.iter().map(|c| c.kind).collect();
    assert_eq!(kinds[0], weeds::WeedControlKind::Hoe, "hoe first");
    assert!(kinds[1..].iter().all(|k| *k == weeds::WeedControlKind::Mulch));
    assert_eq!(d.window("tomato", 70.0), (23.0, 41.0), "UMass: 3.3 to 5.8 weeks");
    assert_eq!(d.window("wheat", 120.0), (12.0, 24.0), "Agostinetto 2008");
    assert!((d.default_period.start - 25.0 / 120.0).abs() < 0.01 && (d.default_period.end - 40.0 / 120.0).abs() < 0.01);
    let mut w = d.wssa_losses.clone();
    w.sort_by(f64::total_cmp);
    assert_eq!(w.len(), 9);
    assert_eq!(d.default_max_loss, w[4], "the median of WSSA's nine");
    let sawdust = d.control("mulch_sawdust").unwrap();
    let lb = 0.45359237;
    let ft2 = 0.09290304;
    assert!((sawdust.kg_per_m2.unwrap() - 50.0 * lb / (100.0 * ft2)).abs() < 0.001, "50 lb on 100 sq ft");
    assert!((sawdust.n_tie_g_per_kg - 1000.0 / 50.0).abs() < 1e-9, "1 lb of N per 50 lb");
    let bark = d.control("mulch_bark").unwrap();
    let kg = bark.mulch_kg_per_m2(Some(&items));
    assert!((kg - 19.24).abs() < 0.05, "2.5 inches of bark at items.csv's volume: {kg}");
    assert!((2.0 * 365.0..=4.0 * 365.0).contains(&bark.lasts_days), "Clemson: 2 to 4 years");
}

//! Crops picked over a season (2026-09-26, picking.rs), through the pure
//! functions, the real FarmingSystem tick and the shipped data. Each test was
//! seen red first; its doc comment says what was broken to see it.

use std::collections::HashMap;
use std::sync::Mutex;

use super::gardening_tests::make_store;
use super::picking::{self, HarvestWindows, Plan};
use super::units::PLOT_AREA_KEY;
use super::*;
use crate::ecs::components::{Controllable, CropInstance, CropPicking, CropPollination, CropSoil, SoilMemory};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::inventory::Inventory;

/// The tick length, real seconds, that is `days` garden days at the default
/// growth speed.
fn dt_for(days: f64) -> f32 {
    (days * SECONDS_PER_DAY / f64::from(DEFAULT_CROP_GROWTH_SPEED)) as f32
}

/// The shipped store, with a tomato bed plot of 4.2 m2 (ten tomato plants at
/// 0.418 m2 each), a lettuce plot of 1 m2, and the notices channel.
fn store() -> DataStore {
    let mut data = make_store();
    let areas: HashMap<String, f32> =
        [("tomato_bed".to_string(), 4.2_f32), ("lettuce_bed".to_string(), 1.0)].into_iter().collect();
    data.insert(PLOT_AREA_KEY, areas);
    data.insert("player_notices", Mutex::new(Vec::<String>::new()));
    // Pests off: a plant bearing through its window keeps ticking, and over
    // the weeks these tests jump, pests on the bed would trim its health and
    // so its picks; the picking arithmetic is what is under test here.
    data.insert("garden_pest_severity", Mutex::new(0.0_f32));
    data
}

fn set_realistic(data: &mut DataStore, on: bool) {
    data.insert(picking::MODE_KEY, Mutex::new(on));
}

fn last_stage(data: &DataStore, plant: &str) -> String {
    data.get::<PlantRegistry>("plant_registry").unwrap().get(plant).unwrap().last_stage().to_string()
}

/// A crop of `plant` in unit 0 of `area`, at `stage`.
fn crop(plant: &str, area: Option<&str>, stage: &str) -> CropInstance {
    CropInstance {
        crop_def_id: plant.to_string(),
        growth_stage: stage.to_string(),
        planted_at: 0.0,
        water_level: 1.0,
        health: 100.0,
        tower_id: area.map(str::to_string),
        tower_slot: area.map(|_| 0),
        health_seconds: 0.0,
        growing_seconds: 0.0,
    }
}

/// A player whose pack never fills, in a home whose irrigation runs: a plant
/// bearing through its window keeps drinking (2026-09-26), so without it the
/// days these tests jump over would dry it out and kill it.
fn player(world: &mut hecs::World) -> hecs::Entity {
    let mut inv = Inventory::new(64);
    inv.volume_capacity_l = 1.0e9;
    world.spawn((crate::ecs::components::Irrigator,));
    world.spawn((inv, Controllable))
}

fn count(world: &hecs::World, who: hecs::Entity, item: &str) -> u32 {
    world.get::<&Inventory>(who).unwrap().count_item(item)
}

/// One tick `days` garden days long, pressing Harvest on `press` if given.
fn tick(sys: &mut FarmingSystem, world: &mut hecs::World, data: &DataStore, days: f64, press: Option<hecs::Entity>) {
    if let Some(e) = press {
        *data.get::<Mutex<Option<u64>>>("harvest_request").unwrap().lock().unwrap() = Some(e.to_bits().into());
    }
    sys.tick(world, dt_for(days), data);
}

fn rec(world: &hecs::World, e: hecs::Entity) -> CropPicking {
    world.get::<&CropPicking>(e).map(|r| (*r).clone()).unwrap_or_default()
}

fn tomato_plan() -> Plan {
    HarvestWindows::shipped().plan("tomato").expect("tomato is picked")
}

/// The shipped file parses; every row names a plants.csv crop; every crop
/// the homes grow (both aeroponic towers, the showcase beds and every grow
/// medium's default crop) says how it is harvested; and a picked crop has at
/// least two picks. UMN's tomato row is 36 picks, 1.75 days apart. Seen red
/// by deleting the basil row (basil is in the nutrition tower).
#[test]
fn every_crop_the_homes_grow_says_how_it_is_harvested() {
    let d = HarvestWindows::parse(picking::HARVEST_WINDOWS_RON).expect("parses");
    let data = store();
    let plants = data.get::<PlantRegistry>("plant_registry").unwrap();
    for c in &d.crops {
        assert!(plants.get(&c.plant).is_some(), "{} is not a plants.csv id", c.plant);
        if let Some(p) = d.plan(&c.plant) {
            assert!(p.picks >= 2, "{}: a picked crop is picked more than once", c.plant);
        }
    }
    let listed: std::collections::HashSet<&str> = d.crops.iter().map(|c| c.plant.as_str()).collect();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    let mut grown: Vec<String> = Vec::new();
    for file in ["towers/aeroponic_configs.ron", "world/showcase.ron", "garden/grow_media.ron"] {
        let text = std::fs::read_to_string(root.join(file)).unwrap();
        for key in ["plant: \"", "default_crop: Some(\""] {
            for part in text.split(key).skip(1) {
                grown.push(part.split('"').next().unwrap().to_string());
            }
        }
        if file.starts_with("world/") {
            for line in text.lines().filter(|l| l.trim_start().starts_with('"') && l.contains(':')) {
                grown.push(line.rsplit(':').next().unwrap().trim().trim_end_matches(',').trim_matches('"').to_string());
            }
        }
    }
    grown.sort();
    grown.dedup();
    assert!(grown.len() > 50, "found the homes' crops: {grown:?}");
    let missing: Vec<&String> = grown.iter().filter(|g| plants.get(g).is_some() && !listed.contains(g.as_str())).collect();
    assert!(missing.is_empty(), "grown but not in harvest_windows.ron: {missing:?}");
    assert_eq!(tomato_plan(), Plan { picks: 36, every_days: 1.75 });
}

/// A tomato bed plot of ten plants, picked at every one of its 36 picks,
/// gives its season and no more: ten plants x (yield_min + roll x (yield_max
/// - yield_min)) items, to within the one item the last pick rounds, at the
/// bottom, the top and the middle of the range. The 36th pick leaves the
/// plot empty. Seen red by giving each pick the whole season (`shares /
/// picks` dropped from `pick_items`): the bottom of the range gave 6,531
/// items against its season of 181.4.
#[test]
fn a_picked_tomatos_picks_sum_to_its_season_yield() {
    let data = store();
    let def = data.get::<PlantRegistry>("plant_registry").unwrap().get("tomato").unwrap().clone();
    let ripe = last_stage(&data, "tomato");
    let plan = tomato_plan();
    for roll in [0.0_f32, 1.0, 0.37] {
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let who = player(&mut world);
        let t = world.spawn((crop("tomato", Some("tomato_bed"), &ripe), CropPicking { roll: Some(roll), ..Default::default() }));
        let mut presses = 0;
        let mut first = true;
        while world.get::<&CropInstance>(t).is_ok() && presses < 100 {
            tick(&mut sys, &mut world, &data, if first { 0.0 } else { plan.every_days }, Some(t));
            first = false;
            presses += 1;
        }
        let season = 10.0 * (f64::from(def.yield_min) + f64::from(roll) * f64::from(def.yield_max - def.yield_min));
        let got = f64::from(count(&world, who, "vegetable_tomato_0"));
        assert_eq!(presses, plan.picks, "roll {roll}: one press a pick, and the last empties the plot");
        assert!(got >= season.floor() && got <= season.floor() + 1.0, "roll {roll}: {got} items against a season of {season:.2}");
        assert!(got <= (10.0 * f64::from(def.yield_max)).ceil(), "never more than the top of the range");
    }
}

/// A crop harvested once is harvested once, as before: a ripe 1 m2 lettuce
/// plot (fourteen heads at 1.085 items each) gives its season in one press,
/// leaves its plot empty and never carries picking state, and a second press
/// finds nothing to pick. Seen red by making every crop picked
/// (`HarvestWindows::plan` returned two picks for a Once crop): the lettuce
/// was still standing after the first press.
#[test]
fn a_once_crop_is_harvested_once_as_before() {
    let data = store();
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    let who = player(&mut world);
    let l = world.spawn((crop("lettuce", Some("lettuce_bed"), &last_stage(&data, "lettuce")),));
    tick(&mut sys, &mut world, &data, 0.0, Some(l));
    assert!(world.get::<&CropInstance>(l).is_err(), "harvested whole and gone");
    let got = count(&world, who, "vegetable_lettuce_0");
    assert!((15..=16).contains(&got), "14 heads of 1.085 items: {got}");
    assert_eq!(world.query::<&CropPicking>().iter().count(), 0, "no picking state for a once crop");
    tick(&mut sys, &mut world, &data, 5.0, Some(l));
    assert_eq!(count(&world, who, "vegetable_lettuce_0"), got, "nothing more");
}

/// Picking before the next pick is ripe gives nothing: a tomato bed picked
/// the day it ripens, then pressed again a garden day later (the interval is
/// 1.75), gives no items and takes no share; at 1.75 days it gives the next.
/// The same in Realistic mode. Seen red by letting `ready` ignore `next_pick`
/// in Forgiving mode (the second press took the ripe share again).
#[test]
fn picking_before_the_interval_gives_nothing() {
    for realistic in [false, true] {
        let mut data = store();
        set_realistic(&mut data, realistic);
        let mut sys = FarmingSystem::new();
        let mut world = hecs::World::new();
        let who = player(&mut world);
        let ripe = last_stage(&data, "tomato");
        let t = world.spawn((crop("tomato", Some("tomato_bed"), &ripe), CropPicking { roll: Some(0.5), ..Default::default() }));
        tick(&mut sys, &mut world, &data, 0.0, Some(t));
        let after_first = count(&world, who, "vegetable_tomato_0");
        assert!(after_first >= 8, "the first pick is a 36th of ten plants' season: {after_first}");
        assert_eq!(rec(&world, t).taken, 1);
        tick(&mut sys, &mut world, &data, 1.0, Some(t));
        assert_eq!(count(&world, who, "vegetable_tomato_0"), after_first, "realistic {realistic}: too soon, nothing");
        assert_eq!(rec(&world, t).taken, 1, "realistic {realistic}: no share taken");
        tick(&mut sys, &mut world, &data, 0.75, Some(t));
        assert!(count(&world, who, "vegetable_tomato_0") > after_first, "realistic {realistic}: the next pick came ripe");
        assert_eq!(rec(&world, t).taken, 2);
    }
}

/// A spent plant leaves its plot empty, and the plot keeps its soil. In
/// Forgiving mode the press after the whole window takes every share left
/// (the plant waited) and the plant is gone, its soil remembered for the next
/// crop sown there. In Realistic mode a plant nobody picks is cleared when
/// its window ends, with a notice, and its passed-over shares are lost. And
/// Clear empties a plot at once. Seen red by not emptying the unit when the
/// last pick ends the plant (the `taken.ends_plant` branch in the harvest did
/// nothing): the Forgiving tomato was still standing.
#[test]
fn a_spent_plant_leaves_the_plot_empty() {
    let ripe = {
        let data = store();
        last_stage(&data, "tomato")
    };
    let plan = tomato_plan();
    // Forgiving: one press after the window takes the season and ends it.
    let data = store();
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    let who = player(&mut world);
    let soil = CropSoil { store: Npk::new(1.0, 2.0, 3.0), uptake: 1.0 };
    let t = world.spawn((crop("tomato", Some("tomato_bed"), &ripe), soil, CropPicking { roll: Some(0.0), ..Default::default() }));
    tick(&mut sys, &mut world, &data, plan.window_days() + 5.0, Some(t));
    assert!(world.get::<&CropInstance>(t).is_err(), "Forgiving: the last share picked, the plot is empty");
    assert!(count(&world, who, "vegetable_tomato_0") >= 181, "the whole season came at once");
    let kept = world.query::<&SoilMemory>().iter().next().map(|(_, m)| m.units.get("tomato_bed").and_then(|u| u.get(&0)).copied());
    assert_eq!(kept.flatten(), Some(Npk::new(1.0, 2.0, 3.0)), "the unit remembers its soil");
    // Realistic: nobody picks, the window ends, the plant is cleared.
    let mut data = store();
    set_realistic(&mut data, true);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    let who = player(&mut world);
    let t = world.spawn((crop("tomato", Some("tomato_bed"), &ripe),));
    tick(&mut sys, &mut world, &data, 0.0, None);
    assert!(world.get::<&CropPicking>(t).is_ok(), "seen ripe: its window started");
    tick(&mut sys, &mut world, &data, plan.window_days() - 1.0, None);
    assert!(world.get::<&CropInstance>(t).is_ok(), "still bearing");
    tick(&mut sys, &mut world, &data, 2.0, None);
    assert!(world.get::<&CropInstance>(t).is_err(), "Realistic: its window ended, the plot is empty");
    assert_eq!(count(&world, who, "vegetable_tomato_0"), 0, "nothing was picked");
    let said = std::mem::take(&mut *data.get::<Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap());
    assert!(said.iter().any(|s| s.contains("Finished bearing") && s.contains("tomato")), "{said:?}");
    assert!(said.iter().any(|s| s.contains("past its best")), "told produce passed over: {said:?}");
    // Clear: the player pulls a ripe plant early (the crop card's button).
    let mut data = store();
    picking::register(&mut data);
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    player(&mut world);
    let t = world.spawn((crop("tomato", Some("tomato_bed"), &ripe),));
    sys.tick(&mut world, 0.0, &data);
    assert!(world.get::<&CropInstance>(t).is_ok(), "ripe and bearing");
    picking::publish(&data, false, Some(t.to_bits().into()));
    sys.tick(&mut world, 0.0, &data);
    assert!(world.get::<&CropInstance>(t).is_err(), "Clear empties the plot");
}

/// Seed follows the harvest (survival mode). A zucchini that flowered indoors
/// with nothing to pollinate it sets no fruit and returns no seed over its
/// whole season; 200 pollinated every day return about the 2 each of a full
/// season; and 200 ripe lettuces that spent their season at half health
/// return exactly 1 each (half a full harvest of 2). Seen red by returning
/// SEEDS_PER_FULL_HARVEST whatever was harvested (the rule before
/// 2026-09-26): the unpollinated zucchini gave 2.
#[test]
fn seed_return_follows_the_harvest() {
    let data = store();
    let ripe = last_stage(&data, "zucchini");
    let flowered = |pollinated: f64| CropPollination { flowering_days: 10.0, pollinated_days: pollinated, hand_days_left: 0.0 };
    let long_ripe = || CropPicking { days_ripe: 1000.0, ..Default::default() };
    let seeds = |world: &hecs::World, who: hecs::Entity, plant: &str| count(world, who, &format!("seed_{plant}_0"));
    // Unpollinated: no fruit, no seed.
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    let who = player(&mut world);
    let z = world.spawn((crop("zucchini", Some("ntower_3"), &ripe), flowered(0.0), long_ripe()));
    tick(&mut sys, &mut world, &data, 0.0, Some(z));
    assert!(world.get::<&CropInstance>(z).is_err(), "its whole season in one press");
    assert_eq!(count(&world, who, "vegetable_zucchini_0"), 0, "no fruit set");
    assert_eq!(seeds(&world, who, "zucchini"), 0, "and no seed");
    // Pollinated every day: about two seeds each.
    let bits: Vec<u64> = (0..200)
        .map(|_| world.spawn((crop("zucchini", Some("ntower_3"), &ripe), flowered(10.0), long_ripe())).to_bits().into())
        .collect();
    *data.get::<Mutex<Vec<u64>>>("harvest_many_request").unwrap().lock().unwrap() = bits;
    sys.tick(&mut world, 0.0, &data);
    let got = seeds(&world, who, "zucchini");
    assert!((360..=440).contains(&got), "200 full seasons return about 400 seeds: {got}");
    // Half a season's health: half the seed.
    let lettuce_ripe = last_stage(&data, "lettuce");
    let half = |c: CropInstance| CropInstance { health_seconds: 500.0, growing_seconds: 1000.0, ..c };
    let bits: Vec<u64> =
        (0..200).map(|_| world.spawn((half(crop("lettuce", None, &lettuce_ripe)),)).to_bits().into()).collect();
    *data.get::<Mutex<Vec<u64>>>("harvest_many_request").unwrap().lock().unwrap() = bits;
    sys.tick(&mut world, 0.0, &data);
    assert_eq!(seeds(&world, who, "lettuce"), 200, "one seed each at half health");
}

/// The bulk "Harvest N ready" button picks what is ready. The Garden panel's
/// view counts a ripe tomato with a pick waiting as ready and one between
/// picks as not, and passes a once crop's `mature` through; sending the ready
/// set through harvest_many_request (as the button does) picks each of those
/// and leaves the rest. Seen red by having the view report a ripe picked
/// crop's `mature` (the tomato between picks was then counted ready).
#[test]
fn the_bulk_harvest_picks_ready_picked_crops() {
    let data = store();
    let mut sys = FarmingSystem::new();
    let mut world = hecs::World::new();
    let who = player(&mut world);
    let ripe = last_stage(&data, "tomato");
    let fresh = world.spawn((crop("tomato", Some("tomato_bed"), &ripe), CropPicking::default()));
    let waiting = world.spawn((
        crop("tomato", Some("tomato_bed"), &ripe),
        CropPicking { days_ripe: 0.5, next_pick: 1, taken: 1, roll: Some(0.5), carry: 0.0 },
    ));
    let due = world.spawn((
        crop("tomato", Some("tomato_bed"), &ripe),
        CropPicking { days_ripe: 1.8, next_pick: 1, taken: 1, roll: Some(0.5), carry: 0.0 },
    ));
    let lettuce = world.spawn((crop("lettuce", Some("lettuce_bed"), &last_stage(&data, "lettuce")),));
    let view = picking::GuiView::new(&world, &data);
    assert!(view.ready(fresh, true) && view.ready(due, true) && view.ready(lettuce, true));
    assert!(!view.ready(waiting, true), "between picks: not ready");
    assert!(view.row(waiting).contains("next in"), "{}", view.row(waiting));
    assert_eq!(view.row(lettuce), "", "a once crop has no picking row");
    let ready: Vec<u64> =
        [fresh, waiting, due, lettuce].into_iter().filter(|e| view.ready(*e, true)).map(|e| e.to_bits().into()).collect();
    *data.get::<Mutex<Vec<u64>>>("harvest_many_request").unwrap().lock().unwrap() = ready;
    sys.tick(&mut world, 0.0, &data);
    assert_eq!(rec(&world, fresh).taken, 1, "the fresh tomato gave its first pick");
    assert_eq!(rec(&world, due).taken, 2, "the due one its second");
    assert_eq!(rec(&world, waiting).taken, 1, "the waiting one untouched");
    assert!(world.get::<&CropInstance>(lettuce).is_err(), "the lettuce harvested whole");
    assert!(count(&world, who, "vegetable_tomato_0") > 0);
}

/// Realistic mode, through the pure functions: a share not picked before the
/// next comes ripe passes over and is lost, the card says so, and the plant
/// is spent when its window ends however little was picked; Forgiving keeps
/// every share waiting. Seen red by making `frontier` ignore `due` in
/// Realistic mode (nothing ever passed over).
#[test]
fn realistic_picking_loses_what_is_not_picked_in_time() {
    let plan = Plan { picks: 4, every_days: 2.0 };
    let mut r = CropPicking::default();
    assert_eq!(picking::take(&mut r, &plan, true), 1, "pick 0 the day it ripens");
    r.days_ripe = 6.5; // picks 1 and 2 came and went; 3 is ripe
    assert_eq!(picking::passed_over(&r, &plan, true), 2);
    assert_eq!(picking::left(&r, &plan, true), 1);
    assert!(picking::card_row(&plan, Some(&r), true, true).contains("2 went past their best unpicked"));
    assert_eq!(picking::take(&mut r, &plan, true), 1, "only the ripe one");
    assert!(picking::spent(&r, &plan, true), "its last share taken");
    let mut f = CropPicking { next_pick: 1, taken: 1, days_ripe: 6.5, ..Default::default() };
    assert_eq!(picking::passed_over(&f, &plan, false), 0, "Forgiving loses nothing");
    assert_eq!(picking::take(&mut f, &plan, false), 3, "Forgiving takes every waiting share");
    assert!(picking::spent(&f, &plan, false));
    let idle = CropPicking { days_ripe: 8.0, ..Default::default() };
    assert!(picking::spent(&idle, &plan, true), "Realistic: the window is over");
    assert!(!picking::spent(&idle, &plan, false), "Forgiving: it waits for the last pick");
}

/// The home's food model counts a picked crop's season once per window, not
/// once per growth_days: a tower basil cup supplies its season per 30 + 266
/// garden days, and a potato (harvested once) per its growth days as before.
/// Seen red by dividing by `growth_days` again in
/// `self_sufficiency::food_supply_kcal_per_day`: the basil cup counted about
/// ten times as much.
#[test]
fn the_food_model_counts_a_picked_season_once_per_window() {
    use crate::systems::self_sufficiency::{food_supply_kcal_per_day, CropNutrition};
    let nutrition = CropNutrition::load(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("food").join("crop_nutrition.ron"),
    )
    .unwrap();
    let data = store();
    let plants = data.get::<PlantRegistry>("plant_registry").unwrap();
    let items = data.get::<crate::systems::inventory::ItemRegistry>("item_registry").unwrap();
    for (plant, window) in [("basil", 266.0), ("potato", 0.0)] {
        let Some(n) = nutrition.get(plant) else { continue };
        let def = plants.get(plant).unwrap();
        let kg = units::plot_harvest_kg(plant, None, Some(plants), Some(items));
        let want = kg * 10.0 * f64::from(n.calories_per_100g) / (f64::from(def.growth_days) + window);
        let got = f64::from(food_supply_kcal_per_day(&[(plant.to_string(), None, 1.0)], &nutrition, plants, items));
        assert!((got - want).abs() < 1e-3 * want.max(1e-6), "{plant}: {got} kcal/day, want {want}");
    }
    assert!(nutrition.get("basil").is_some(), "basil is in the food table, so the picked case ran");
}

/// The whole-number bookkeeping: whole items come out of each pick with the
/// fraction carried, the last pick rounds the remainder, and seed follows the
/// share harvested. Seen red by resetting the carry on every pick (a season
/// of 36 picks of 0.9 items then gave nothing).
#[test]
fn picks_carry_their_fractions_and_the_last_rounds() {
    let mut carry = 0.0;
    let mut total = 0;
    for k in 0..36 {
        total += picking::pick_items(&mut carry, 32.4, 1, 36, k == 35, 0.99);
    }
    assert_eq!(total, 32, "36 picks of 0.9 items give the season's 32 (0.4 left, rounded down at 0.99)");
    let mut carry = 0.0;
    assert_eq!(picking::pick_items(&mut carry, 32.4, 36, 36, true, 0.1), 33, "one take of it all rounds 0.4 up at 0.1");
    assert_eq!(picking::seed_count(0.0, 10.0, 0.0), 0);
    assert_eq!(picking::seed_count(10.0, 10.0, 0.99), 2, "a full season, 2");
    assert_eq!(picking::seed_count(5.0, 10.0, 0.99), 1, "half, 1");
}

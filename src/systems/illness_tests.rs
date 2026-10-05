//! Being ill, and the medical items' Use button (BUG-162, 2026-10-05).
//!
//! The stomach illness from spoiled or raw food used to take 3 health every 15 s for 90
//! minutes, which emptied a full health bar in about 8 minutes, and nothing removed it; and
//! the Inventory's Use button did nothing, so no medical item could be used. Now the illness
//! takes WATER for a day or two of game time and passes by itself, drinking puts the water
//! back (oral rehydration solution best), harm comes only from going without, and Use does
//! what each medical item's data says (data/medical/treatments.ron).
//!
//! A child of `food` (`use super::*`): every test drives the FoodSystem the way the game
//! does, through its request channels, so the tests read the data files themselves and check
//! that the game did what the files say.
//!
//! RED CHECKS. Each "Seen red" below, unless it says otherwise, was run on 2026-10-05 against
//! the code before the fix (main at ee2947a79: food.rs, status_effects.rs and the Use button
//! as they were), with this change's new data in place (treatments.ron, illnesses.ron, the
//! rehydration items and the effect tags) but status_effects.csv's Food Poisoning row and its
//! `dispel_type` column as they were, and `systems::illness` holding only its `Mode`.

use super::*;
use crate::ecs::components::{Controllable, Dead, Health, StatusEffects, Vitals};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::illness::{Mode, MODE_KEY};
use crate::systems::inventory::Inventory;
use crate::systems::status_effects::StatusEffectRegistry;
use std::sync::Mutex;

fn data_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/data"))
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(data_dir().join(rel)).unwrap_or_else(|e| panic!("data/{rel}: {e}"))
}

fn registry() -> StatusEffectRegistry {
    StatusEffectRegistry::from_csv(read("status_effects.csv").as_bytes()).expect("status_effects.csv")
}

// ── The two data files, read here on their own so the tests check the game against them ──

#[derive(serde::Deserialize)]
struct TreatmentsFile {
    treatments: Vec<TreatmentRow>,
}

#[derive(serde::Deserialize, Clone)]
struct TreatmentRow {
    item: String,
    #[serde(default)]
    heals: f32,
    #[serde(default)]
    ends: Vec<String>,
    #[serde(default)]
    used: String,
    #[serde(default)]
    no_help: String,
}

fn treatments() -> Vec<TreatmentRow> {
    ron::from_str::<TreatmentsFile>(&read("medical/treatments.ron")).expect("treatments.ron parses").treatments
}

#[derive(serde::Deserialize)]
struct IllnessesFile {
    forgiving: SharesRow,
    illnesses: Vec<IllnessRow>,
}

#[derive(serde::Deserialize)]
struct SharesRow {
    course_share: f32,
    water_share: f32,
}

#[derive(serde::Deserialize)]
struct IllnessRow {
    effect: String,
    water_l_per_day: f32,
    onset: String,
    helps: String,
    passed: String,
    #[serde(default)]
    again: String,
}

fn illnesses() -> IllnessesFile {
    ron::from_str(&read("medical/illnesses.ron")).expect("illnesses.ron parses")
}

fn food_poisoning() -> IllnessRow {
    illnesses().illnesses.into_iter().find(|i| i.effect == "food_poisoning").expect("food_poisoning in illnesses.ron")
}

// ── A world with one player, wired the way lib.rs wires the FoodSystem ──

/// The DataStore as lib.rs wires it for the FoodSystem; `mode` is the Settings > Gameplay >
/// Illness mode (absent, as in a headless test, is Realistic).
fn store(mode: Option<Mode>) -> DataStore {
    let mut data = DataStore::new();
    data.insert("status_effect_registry", registry());
    for slot in ["consume_request", "drink_request", "use_item_request"] {
        data.insert(slot, Mutex::new(Option::<String>::None));
    }
    data.insert("rest_request", Mutex::new(false));
    data.insert("compost_request", Mutex::new(false));
    data.insert("player_death", Mutex::new(Option::<String>::None));
    data.insert("player_notices", Mutex::new(Vec::<String>::new()));
    if let Some(mode) = mode {
        data.insert(MODE_KEY, mode);
    }
    data
}

fn vitals(satiation: f32, hydration: f32) -> Vitals {
    Vitals { satiation, hydration, ..Vitals::default() }
}

/// A healthy adult at full health. Satiation 40 so a meal does not make them Well Fed, whose
/// healing would hide any harm.
fn adult(world: &mut hecs::World, hydration: f32) -> hecs::Entity {
    world.spawn((Controllable, Inventory::new(36), vitals(40.0, hydration), StatusEffects::default(), Health::default()))
}

fn request(data: &DataStore, channel: &str, item_id: &str) {
    *data.get::<Mutex<Option<String>>>(channel).unwrap().lock().unwrap() = Some(item_id.to_string());
}

fn notices(data: &DataStore) -> Vec<String> {
    std::mem::take(&mut *data.get::<Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap())
}

fn give(world: &mut hecs::World, who: hecs::Entity, item_id: &str, qty: u32) {
    world.get::<&mut Inventory>(who).unwrap().add_item(item_id, qty, 99);
}

/// Make `who` ill the way the game does: they eat a spoiled roast chicken, and spoiled food
/// always poisons.
fn fall_ill(sys: &mut FoodSystem, world: &mut hecs::World, data: &DataStore, who: hecs::Entity) {
    let keeps = sys.freshness_secs("roast_chicken_0").expect("roast chicken is food");
    {
        let mut inv = world.get::<&mut Inventory>(who).unwrap();
        inv.add_item("roast_chicken_0", 1, 99);
        let stack = inv.slots.iter_mut().flatten().find(|s| s.item_id == "roast_chicken_0").unwrap();
        stack.age_s = keeps + 1.0;
    }
    request(data, "consume_request", "roast_chicken_0");
    sys.tick(world, 0.0, data);
    assert!(world.get::<&StatusEffects>(who).unwrap().has("food_poisoning"), "spoiled food made them ill");
}

fn ill(world: &hecs::World, who: hecs::Entity) -> bool {
    world.get::<&StatusEffects>(who).unwrap().has("food_poisoning")
}

fn hydration(world: &hecs::World, who: hecs::Entity) -> f32 {
    world.get::<&Vitals>(who).unwrap().hydration
}

fn health(world: &hecs::World, who: hecs::Entity) -> f32 {
    world.get::<&Health>(who).unwrap().current
}

/// Drink one `item_id` (given first), returning the Hydration it put back.
fn drink(sys: &mut FoodSystem, world: &mut hecs::World, data: &DataStore, who: hecs::Entity, item_id: &str) -> f32 {
    give(world, who, item_id, 1);
    let before = hydration(world, who);
    request(data, "drink_request", item_id);
    sys.tick(world, 0.0, data);
    hydration(world, who) - before
}

/// Ten game minutes, the step every long run below takes.
const STEP_S: f32 = 600.0;
/// Steps in a game hour.
const STEPS_PER_HOUR: u32 = 6;

/// Tick `who`'s world in ten-minute steps until the illness has passed (or four days have),
/// calling `each` before every step with the step's number. Returns the hours it lasted.
fn until_well(
    sys: &mut FoodSystem,
    world: &mut hecs::World,
    data: &DataStore,
    who: hecs::Entity,
    mut each: impl FnMut(u32, &mut FoodSystem, &mut hecs::World),
) -> f32 {
    let mut step = 0u32;
    while ill(world, who) && step < 96 * STEPS_PER_HOUR {
        each(step, sys, world);
        sys.tick(world, STEP_S, data);
        step += 1;
    }
    step as f32 / STEPS_PER_HOUR as f32
}

// ── The illness ──

/// THE BUG: an untreated healthy adult with food poisoning is alive an hour later, with all
/// their health, and still ill (it lasts a day or two, not 90 minutes).
///
/// Seen red on the old code: "an hour of food poisoning killed a healthy adult" (3 health
/// every 15 s emptied 100 health in about 8 minutes).
#[test]
fn an_untreated_healthy_adult_with_food_poisoning_is_alive_after_an_hour() {
    let mut sys = FoodSystem::new(data_dir());
    let data = store(None);
    let mut world = hecs::World::new();
    let p = adult(&mut world, 80.0);
    fall_ill(&mut sys, &mut world, &data, p);
    for _ in 0..60 {
        sys.tick(&mut world, 60.0, &data);
    }
    assert!(world.get::<&Dead>(p).is_err(), "an hour of food poisoning killed a healthy adult");
    assert!(health(&world, p) >= 99.9, "the illness itself does no harm: health {}", health(&world, p));
    assert!(ill(&world, p), "still ill after an hour: it lasts a day or more");
}

/// It acts on the body's water: over twelve game hours an ill adult loses the illness's
/// `water_l_per_day` share on top of what a well one loses, and nothing else.
///
/// Seen red on the old code: "the illness took -24.7 points of water in 12 h, the data says
/// 15.0": it took no water at all, and it killed the ill adult within minutes, after which a
/// dead body loses none, while the well one lost its 24.7.
#[test]
fn food_poisoning_takes_the_water_its_data_says() {
    let fp = food_poisoning();
    let says = fp.water_l_per_day * HYDRATION_PER_LITRE * 0.5;
    let taken = water_taken_in_12_hours(Mode::Realistic);
    assert!((taken - says).abs() < 0.05, "the illness took {taken:.1} points of water in 12 h, the data says {says:.1}");
}

/// Hydration points the illness alone takes in its first twelve game hours, in `mode`: what
/// an ill adult loses less what a well one loses in the same twelve hours (the daily clock and
/// a little sweat, alike for both), both starting full after the same meal, the ill one's
/// spoiled.
fn water_taken_in_12_hours(mode: Mode) -> f32 {
    let run = |sick: bool| {
        let mut sys = FoodSystem::new(data_dir());
        let data = store(Some(mode));
        let mut world = hecs::World::new();
        let p = adult(&mut world, 100.0);
        if sick {
            fall_ill(&mut sys, &mut world, &data, p);
        } else {
            give(&mut world, p, "roast_chicken_0", 1);
            request(&data, "consume_request", "roast_chicken_0");
            sys.tick(&mut world, 0.0, &data);
        }
        let start = hydration(&world, p);
        for _ in 0..12 * STEPS_PER_HOUR {
            sys.tick(&mut world, STEP_S, &data);
        }
        start - hydration(&world, p)
    };
    run(true) - run(false)
}

/// Drinking puts the water back, and oral rehydration solution puts back more than plain
/// water, which puts back more than a sugary drink; a well person keeps all of every drink.
///
/// Seen red on the old code: "while ill, oral rehydration solution put back 30.0 points and
/// water 30.0: the solution must put back more" (a drink was a drink, ill or well).
#[test]
fn while_ill_oral_rehydration_solution_puts_back_more_than_water() {
    let mut sys = FoodSystem::new(data_dir());
    let data = store(None);
    let mut world = hecs::World::new();
    let p = adult(&mut world, 20.0);
    // Each drink from the same 20 points, so none is cut short by a full bar.
    let from_20 = |sys: &mut FoodSystem, world: &mut hecs::World, item: &str| {
        world.get::<&mut Vitals>(p).unwrap().hydration = 20.0;
        drink(sys, world, &data, p, item)
    };
    let well_water = from_20(&mut sys, &mut world, "water_bottle_0");
    fall_ill(&mut sys, &mut world, &data, p);
    let ors = from_20(&mut sys, &mut world, "ors_solution_0");
    let water = from_20(&mut sys, &mut world, "water_bottle_0");
    let juice = from_20(&mut sys, &mut world, "juice_0");
    assert!(ors > water, "while ill, oral rehydration solution put back {ors:.1} points and water {water:.1}: the solution must put back more");
    assert!(water > juice && juice > 0.0, "while ill, water put back {water:.1} and juice {juice:.1}: a sugary drink helps least");
    assert!((well_water - DRINK_HYDRATION).abs() < 0.01, "a well person keeps all of a drink: {well_water:.1}");
    assert!((ors - DRINK_HYDRATION).abs() < 0.01, "oral rehydration solution keeps all of its water: {ors:.1}");
}

/// Harm comes only from not drinking. Through the whole Realistic course, an adult who drinks
/// a bottle of water every six hours never runs dry and keeps every point of health; one who
/// drinks nothing runs dry within about a day and is hurt by the end.
///
/// Seen red on the old code: "the adult who drank was hurt: health 0.0" (the illness drained
/// health whatever anyone drank).
#[test]
fn drinking_through_food_poisoning_keeps_a_body_from_harm() {
    let run = |drinks: bool| {
        let mut sys = FoodSystem::new(data_dir());
        let data = store(Some(Mode::Realistic));
        let mut world = hecs::World::new();
        let p = adult(&mut world, 80.0);
        fall_ill(&mut sys, &mut world, &data, p);
        let mut lowest = f32::MAX;
        let hours = until_well(&mut sys, &mut world, &data, p, |step, sys, world| {
            if drinks && step % (6 * STEPS_PER_HOUR) == 0 {
                drink(sys, world, &data, p, "water_bottle_0");
            }
            lowest = lowest.min(hydration(world, p));
        });
        (health(&world, p), lowest.min(hydration(&world, p)), hours)
    };
    let (drank_health, drank_lowest, course_h) = run(true);
    let (dry_health, dry_lowest, _) = run(false);
    assert!(drank_health >= 99.9, "the adult who drank was hurt: health {drank_health:.1}");
    assert!(drank_lowest > 20.0, "the adult who drank never ran low: lowest Hydration {drank_lowest:.1}");
    assert!(dry_lowest <= 0.0, "drinking nothing ran the body dry: lowest Hydration {dry_lowest:.1}");
    assert!(dry_health < 50.0, "drinking nothing does harm by the end: health {dry_health:.1}");
    assert!((36.0..=60.0).contains(&course_h), "the Realistic course lasts about two days: {course_h:.1} h");
}

/// The simplified mode is milder: Forgiving (the default) runs a shorter course and takes
/// less water than Realistic, by the shares in illnesses.ron.
///
/// Seen red on the old code: "Forgiving took -24.7 points of water in 12 h and Realistic
/// -24.7: Forgiving must take less": there was no mode, the illness took no water, and it
/// killed within minutes in both.
#[test]
fn the_simplified_mode_is_milder_than_realistic() {
    let shares = illnesses().forgiving;
    let real_taken = water_taken_in_12_hours(Mode::Realistic);
    let gentle_taken = water_taken_in_12_hours(Mode::Forgiving);
    assert!(
        gentle_taken < real_taken,
        "Forgiving took {gentle_taken:.1} points of water in 12 h and Realistic {real_taken:.1}: Forgiving must take less"
    );
    assert!(
        (gentle_taken - real_taken * shares.water_share).abs() < 0.05,
        "Forgiving takes {} of Realistic's water: {gentle_taken:.1} of {real_taken:.1}",
        shares.water_share
    );
    let course = |mode: Mode| {
        let mut sys = FoodSystem::new(data_dir());
        let data = store(Some(mode));
        let mut world = hecs::World::new();
        let p = adult(&mut world, 100.0);
        fall_ill(&mut sys, &mut world, &data, p);
        until_well(&mut sys, &mut world, &data, p, |_, _, _| {})
    };
    let (real_h, gentle_h) = (course(Mode::Realistic), course(Mode::Forgiving));
    assert!(gentle_h < real_h, "Forgiving's course ({gentle_h:.1} h) is shorter than Realistic's ({real_h:.1} h)");
    assert!(
        (gentle_h - real_h * shares.course_share).abs() <= 0.2,
        "Forgiving lasts {} of Realistic's course: {gentle_h:.1} h of {real_h:.1} h",
        shares.course_share
    );
}

/// The player is told in plain words when it starts (and what helps) and when it passes.
///
/// Seen red on the old code: "no notice when the illness started: []".
#[test]
fn food_poisoning_tells_the_player_when_it_starts_and_when_it_passes() {
    let fp = food_poisoning();
    let mut sys = FoodSystem::new(data_dir());
    let data = store(Some(Mode::Forgiving));
    let mut world = hecs::World::new();
    let p = adult(&mut world, 100.0);
    fall_ill(&mut sys, &mut world, &data, p);
    let said = notices(&data);
    assert!(said.iter().any(|n| n.contains(&fp.onset) && n.contains(&fp.helps)), "no notice when the illness started: {said:?}");
    let mut passed = Vec::new();
    until_well(&mut sys, &mut world, &data, p, |_, _, _| passed.extend(notices(&data)));
    passed.extend(notices(&data));
    assert!(!ill(&world, p), "the illness passed by itself");
    assert!(passed.iter().any(|n| n.contains(&fp.passed)), "no notice when it passed: {passed:?}");
}

/// A death from drying out while the illness took water says so, and points at what to do:
/// the death screen reads "dehydration from Food Poisoning", not plain "dehydration".
///
/// Seen red with the cause's illness line removed from food.rs: "the death line names the
/// illness: Some(\"dehydration\")".
#[test]
fn drying_out_from_food_poisoning_says_so_on_the_death_screen() {
    let mut sys = FoodSystem::new(data_dir());
    let data = store(Some(Mode::Realistic));
    let mut world = hecs::World::new();
    let p = adult(&mut world, 100.0);
    fall_ill(&mut sys, &mut world, &data, p);
    {
        let mut v = world.get::<&mut Vitals>(p).unwrap();
        v.hydration = 0.0;
    }
    world.get::<&mut Health>(p).unwrap().current = 1.0;
    for _ in 0..STEPS_PER_HOUR {
        sys.tick(&mut world, STEP_S, &data);
    }
    assert!(world.get::<&Dead>(p).is_ok(), "an hour without water at 1 health is fatal");
    let cause = data.get::<Mutex<Option<String>>>("player_death").unwrap().lock().unwrap().clone();
    assert_eq!(cause.as_deref(), Some("dehydration from Food Poisoning"), "the death line names the illness: {cause:?}");
}

/// ILL AGAIN WHILE STILL ILL (2026-10-05, item 7 of the second seam review): eating spoiled
/// food two hours before Food Poisoning would pass does not start it over. It passes when it
/// was going to, and the player is told so in plain words, with the time it still has to run
/// (illnesses.ron's `again` line and `again_adds_h`, 0 for Food Poisoning).
///
/// Seen red on main at 347c8f77b (with this change's illnesses.ron): "ill again two hours
/// before the end: it had not passed 24 h later (it was due in 2 h), and the player was told
/// \"\"" (the course started over, without a word).
#[test]
fn eating_spoiled_food_again_while_ill_does_not_start_the_illness_over() {
    let fp = food_poisoning();
    let mut sys = FoodSystem::new(data_dir());
    let mut data = store(Some(Mode::Realistic));
    // Thirst held still (the Vitals drain slider at 0): this is about the course, not the water.
    data.insert("vitals_drain_scale", Mutex::new(0.0_f32));
    let mut world = hecs::World::new();
    let p = adult(&mut world, 100.0);
    fall_ill(&mut sys, &mut world, &data, p);
    notices(&data);
    let course_s = registry().duration("food_poisoning");
    let mut t = 0.0_f32;
    while t < course_s - 2.0 * 3600.0 {
        sys.tick(&mut world, STEP_S, &data);
        t += STEP_S;
    }
    assert!(ill(&world, p), "the setup: still ill two hours before the end");
    fall_ill(&mut sys, &mut world, &data, p);
    let said = notices(&data).join(" ");
    let mut after = 0.0_f32;
    while ill(&world, p) && after < 24.0 * 3600.0 {
        sys.tick(&mut world, 60.0, &data);
        after += 60.0;
    }
    let passed = if ill(&world, p) { "had not passed 24 h later".to_string() } else { format!("passed {:.2} h later", after / 3600.0) };
    let on_time = !ill(&world, p) && (after - 2.0 * 3600.0).abs() <= 60.0;
    let told = !fp.again.is_empty() && said.contains(&fp.again) && said.contains("2 hours");
    assert!(
        on_time && told,
        "ill again two hours before the end: it {passed} (it was due in 2 h), and the player was told {said:?}"
    );
}

// ── The medical items' Use button ──

/// Use on each medical item does exactly what treatments.ron says: one that helps is used up,
/// restores its Health and ends every effect carrying a tag it names, and nothing else; one
/// that would do nothing is kept, changes nothing, and says why.
///
/// Seen red on the old code: "Use on bandage_0: 2 left, expected 1 (used up)" (the Use
/// button discarded its click).
#[test]
fn use_on_each_medical_item_does_what_its_data_says() {
    let reg = registry();
    let rows = treatments();
    assert!(rows.len() >= 17, "every medical item has a row: {}", rows.len());
    for t in rows {
        let mut sys = FoodSystem::new(data_dir());
        let data = store(None);
        let mut world = hecs::World::new();
        let p = adult(&mut world, 80.0);
        world.get::<&mut Health>(p).unwrap().current = 50.0;
        // The effects it ends (one carrying each tag it names), and Food Poisoning, which no
        // medical item ends.
        let mut ends_ids: Vec<String> = Vec::new();
        {
            let mut fx = world.get::<&mut StatusEffects>(p).unwrap();
            fx.apply("food_poisoning", 3600.0);
            for tag in &t.ends {
                let def = reg
                    .effects
                    .values()
                    .find(|d| d.tags.split('|').any(|x| x == tag))
                    .unwrap_or_else(|| panic!("{}: no status effect carries the tag {tag}", t.item));
                fx.apply(&def.id, 600.0);
                ends_ids.push(def.id.clone());
            }
        }
        give(&mut world, p, &t.item, 2);
        request(&data, "use_item_request", &t.item);
        sys.tick(&mut world, 0.0, &data);

        let left = world.get::<&Inventory>(p).unwrap().count_item(&t.item);
        let fx = world.get::<&StatusEffects>(p).unwrap().clone();
        let said = notices(&data).join(" ");
        let helps = t.heals > 0.0 || !t.ends.is_empty();
        if helps {
            assert_eq!(left, 1, "Use on {}: {left} left, expected 1 (used up)", t.item);
            let want = (50.0 + t.heals).min(100.0);
            assert!((health(&world, p) - want).abs() < 0.01, "Use on {}: health {} not {want}", t.item, health(&world, p));
            for id in &ends_ids {
                assert!(!fx.has(id), "Use on {} left {id}, which carries a tag it ends", t.item);
            }
            assert!(said.contains(&t.used), "Use on {} said {said:?}, not its `used` line", t.item);
        } else {
            assert_eq!(left, 2, "Use on {}: it does nothing here, so it is kept ({left} left)", t.item);
            assert!((health(&world, p) - 50.0).abs() < 0.01, "Use on {} changed health", t.item);
            assert!(said.contains(&t.no_help), "Use on {} said {said:?}, not why it does nothing", t.item);
        }
        assert!(fx.has("food_poisoning"), "Use on {} ended Food Poisoning, which no medical item ends", t.item);
    }
}

/// Antibiotics do not end food poisoning: they are kept, and the player is told what helps
/// instead. They do end an infection caused by bacteria.
///
/// Seen red on the old code: "the player is told what helps instead: """ (Use did nothing,
/// so nothing was said, and the infection stayed too).
#[test]
fn antibiotics_do_not_end_food_poisoning_but_end_a_bacterial_infection() {
    let fp = food_poisoning();
    let mut sys = FoodSystem::new(data_dir());
    let data = store(None);
    let mut world = hecs::World::new();
    let p = adult(&mut world, 80.0);
    fall_ill(&mut sys, &mut world, &data, p);
    notices(&data);
    give(&mut world, p, "antibiotics_0", 1);
    request(&data, "use_item_request", "antibiotics_0");
    sys.tick(&mut world, 0.0, &data);
    assert!(ill(&world, p), "antibiotics ended Food Poisoning");
    assert_eq!(world.get::<&Inventory>(p).unwrap().count_item("antibiotics_0"), 1, "antibiotics that cannot help are kept");
    let said = notices(&data).join(" ");
    assert!(said.contains(&fp.helps), "the player is told what helps instead: {said:?}");

    // An infection caused by bacteria (data/status_effects.csv tags it "bacterial").
    assert!(registry().get("infection").is_some_and(|d| d.tags.split('|').any(|t| t == "bacterial")), "Infection is bacterial");
    world.get::<&mut StatusEffects>(p).unwrap().apply("infection", 600.0);
    request(&data, "use_item_request", "antibiotics_0");
    sys.tick(&mut world, 0.0, &data);
    let fx = world.get::<&StatusEffects>(p).unwrap().clone();
    assert!(!fx.has("infection"), "antibiotics left the bacterial Infection");
    assert!(fx.has("food_poisoning"), "and still not the Food Poisoning");
    assert_eq!(world.get::<&Inventory>(p).unwrap().count_item("antibiotics_0"), 0, "used up on the infection");
}

// ── The data and the code agree ──

/// No column of status_effects.csv is silently ignored: every row of the shipped file loads,
/// and a row with a column the game does not read is refused, so a new column has to be
/// given a field (and a use) before its rows load.
///
/// Seen red on the old code: "a row with a column nothing reads loaded: 1 effect(s)" (unknown
/// columns, `dispel_type` among them, were dropped without a word).
#[test]
fn every_status_effect_column_is_read() {
    let text = read("status_effects.csv");
    let rows = text.lines().filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#')).count() - 1;
    assert_eq!(registry().len(), rows, "every row of status_effects.csv loads");
    let header = text.lines().find(|l| l.starts_with("id,")).expect("the header");
    let row = text.lines().find(|l| l.starts_with("well_fed,")).expect("the well_fed row");
    let extra = format!("{header},mystery\n{row},1\n");
    let reg = StatusEffectRegistry::from_csv(extra.as_bytes()).expect("parses");
    assert_eq!(reg.len(), 0, "a row with a column nothing reads loaded: {} effect(s)", reg.len());
}

/// Every medical item has a row in treatments.ron (so none shows a button that does nothing),
/// every row names a real item, and every tag a row ends is carried by some status effect.
#[test]
fn every_medical_item_has_a_treatment_and_every_treatment_is_real() {
    #[derive(serde::Deserialize)]
    struct ItemRow {
        id: String,
        #[serde(default)]
        category: String,
    }
    let items: Vec<ItemRow> = crate::assets::loader::parse_csv(read("items.csv").as_bytes()).expect("items.csv");
    let rows = treatments();
    let reg = registry();
    for item in items.iter().filter(|i| i.category == "medical") {
        assert!(rows.iter().any(|t| t.item == item.id), "{} is a medical item with no row in treatments.ron", item.id);
    }
    for t in &rows {
        assert!(items.iter().any(|i| i.id == t.item), "treatments.ron names {}, which is not an item", t.item);
        assert!(!t.no_help.is_empty(), "{} says nothing when it cannot help", t.item);
        if t.heals <= 0.0 && t.ends.is_empty() {
            assert!(t.used.is_empty(), "{} can never be used, so it needs no `used` line", t.item);
        }
        for tag in &t.ends {
            assert!(
                reg.effects.values().any(|d| d.tags.split('|').any(|x| x == tag)),
                "{} ends the tag {tag}, which no status effect carries",
                t.item
            );
        }
    }
    assert_eq!(rows.iter().filter(|t| t.item == "antibiotics_0").count(), 1);
    let fp_tags = reg.get("food_poisoning").expect("food_poisoning").tags.clone();
    let antibiotics = rows.iter().find(|t| t.item == "antibiotics_0").expect("antibiotics_0");
    for tag in &antibiotics.ends {
        assert!(!fp_tags.split('|').any(|x| x == tag), "antibiotics end the tag {tag}, which Food Poisoning carries");
    }
}

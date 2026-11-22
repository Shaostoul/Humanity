//! Preserving rung 1's tests (2026-11-21, docs/design/preserving.md): the drying and canning model
//! on the shipped data, the tested tables against the guide's figures, every preserving recipe
//! decided, weights and volumes following the data, the preserved state on a stack, and the systems
//! it reaches: a crafted jar that comes out with a botulism risk, the spoilage clock of an eaten
//! food, and Botulism itself.
//!
//! RED FIRST, 2026-11-21: with ten faults put in on purpose (preserving.rs, food.rs, crafting/mod.rs
//! and the data), one run (`cargo test -p humanity-systems --lib preserving`, "1 passed; 10
//! failed") showed every test below red but the byte round trip. Each test's comment gives its
//! fault and what it said.

use super::*;
use crate::crafting::{CraftingSystem, RecipeRegistry};
use crate::inventory::{Inventory, ItemRegistry};
use humanity_core::ecs::components::{Controllable, Health, StatusEffects, Vitals};
use humanity_core::ecs::systems::System;

fn data_dir() -> &'static Path {
    Path::new(concat!(env!("HUMANITY_REPO_ROOT"), "/data"))
}

fn shipped() -> Preserving {
    let p = Preserving::load(data_dir());
    assert!(!p.data.procedures.is_empty() && !p.data.drying_classes.is_empty(), "food_system.ron's preserving tables load");
    p
}

fn proc_of<'a>(p: &'a Preserving, recipe: &str) -> &'a Procedure {
    p.procedure(recipe).unwrap_or_else(|| panic!("{recipe} is a preserving procedure"))
}

fn choice(recipe: &str, method: &str, time: f32, psi: f32, vented: bool) -> Choice {
    Choice { recipe: recipe.into(), method: method.into(), time, psi, vented }
}

// -- The model ------------------------------------------------------------------------------------

/// DRYING TAKES ITS TIME AND THE FOOD'S WATER: apple rings in the oven are dry (20% water or less,
/// NCHFP) only after the time the water curve gives, which falls inside Colorado State's 6 to 24
/// hours; taken off at 4 hours they are still wet and spoil; carrots inside 6 to 10. Red with
/// dry_outcome judging "dry" by the method alone (every oven result safe): "4 hours in the oven
/// left apples at 62% water, yet came out Safe".
#[test]
fn drying_follows_the_time_and_the_food_s_water() {
    let p = shipped();
    let apples = proc_of(&p, "dry_apples");
    let early = p.dry_outcome(apples, "oven", 4.0, 20.0).unwrap();
    assert!(
        early.state == PreserveState::Spoils && early.reason == Reason::NotDry,
        "4 hours in the oven left apples at {:.0}% water, yet came out {:?}",
        early.water.unwrap() * 100.0,
        early.state
    );
    let ready = early.ready.expect("the oven dries apples");
    assert!((6.0..=24.0).contains(&ready), "apples dry in {ready:.1} h, outside Colorado State's 6 to 24");
    let done = p.dry_outcome(apples, "oven", ready + 0.5, 20.0).unwrap();
    assert_eq!(done.state, PreserveState::Safe, "dried past their time, apples keep: {done:?}");
    assert!(done.water.unwrap() <= 0.20, "and they are at or under NCHFP's 20%: {:?}", done.water);
    let carrots = proc_of(&p, "dry_carrots");
    let r = p.dry_outcome(carrots, "oven", 1.0, 20.0).unwrap().ready.unwrap();
    assert!((6.0..=10.0).contains(&r), "carrots dry in {r:.1} h, outside Colorado State's 6 to 10");
}

/// THE SUN DRIES FRUIT ONLY, AND ONLY WHEN IT IS WARM: NCHFP's 85 F minimum and "Vegetables ... and
/// meats are not recommended for out-of-doors drying". Red with the sun's classes and minimum
/// ignored: "carrots in the sun at 32 C came out Safe" and "apples in the sun at 20 C came out Safe".
#[test]
fn the_sun_dries_fruit_only_and_only_when_it_is_warm() {
    let p = shipped();
    let apples = proc_of(&p, "dry_apples");
    let cool = p.dry_outcome(apples, "sun", 200.0, 20.0).unwrap();
    assert!(cool.state == PreserveState::Spoils && cool.reason == Reason::SunTooCool, "apples in the sun at 20 C came out {cool:?}");
    let hot = p.dry_outcome(apples, "sun", 200.0, 32.0).unwrap();
    assert_eq!(hot.state, PreserveState::Safe, "apples in the sun at 32 C for 200 hours dry: {hot:?}");
    let sun_h = hot.ready.unwrap();
    let oven_h = p.dry_outcome(apples, "oven", 1.0, 20.0).unwrap().ready.unwrap();
    assert!(sun_h > 2.0 * oven_h, "the sun takes several days, far longer than the oven ({sun_h:.0} h against {oven_h:.0} h)");
    let carrots = proc_of(&p, "dry_carrots");
    let veg = p.dry_outcome(carrots, "sun", 200.0, 32.0).unwrap();
    assert!(veg.state == PreserveState::Spoils && veg.reason == Reason::SunNotForThis, "carrots in the sun at 32 C came out {veg:?}");
}

/// A LOW-ACID FOOD WATER-BATH CANNED KEEPS ITS BOTULISM RISK; PRESSURE CANNED BY THE TESTED PROCESS
/// IT DOES NOT. Carrots and potatoes in boiling water for any time, in a pressure canner not vented,
/// under the band's pounds or short of the tested minutes: botulism risk. Vented, 10 lb, the tested
/// minutes: safe. Red with can_outcome accepting any canner that runs the tested minutes: "carrots
/// in a boiling-water canner for 90 minutes came out Safe".
#[test]
fn low_acid_water_bath_keeps_its_botulism_risk_and_pressure_does_not() {
    let p = shipped();
    for recipe in ["can_carrots", "can_potatoes"] {
        let food = proc_of(&p, recipe);
        assert!(!food.acid, "{recipe}: a vegetable is low-acid");
        let bath = p.can_outcome(food, "boiling_water", 90.0, 0.0, false);
        assert_eq!(bath.state, PreserveState::Botulism, "{recipe} in a boiling-water canner for 90 minutes came out {:?}", bath.state);
        assert_eq!(bath.reason, Reason::NoTestedProcess);
        let need = p.sensible(food).unwrap();
        assert_eq!(p.can_outcome(food, "pressure_weighted", need.time, need.psi, true).state, PreserveState::Safe, "{recipe} pressure canned by the tested process is safe");
        let unvented = p.can_outcome(food, "pressure_weighted", need.time, need.psi, false);
        assert_eq!((unvented.state, unvented.reason), (PreserveState::Botulism, Reason::NotVented), "{recipe} not vented");
        let low = p.can_outcome(food, "pressure_weighted", need.time, need.psi - 5.0, true);
        assert_eq!((low.state, low.reason), (PreserveState::Botulism, Reason::PressureLow), "{recipe} at {} lb", need.psi - 5.0);
        let short = p.can_outcome(food, "pressure_weighted", need.time - 5.0, need.psi, true);
        assert_eq!((short.state, short.reason), (PreserveState::Botulism, Reason::TooShort), "{recipe} 5 minutes short");
    }
    // An acid food under-processed spoils; it is not a botulism risk at pH 4.6 or lower.
    let sauce = proc_of(&p, "can_applesauce");
    assert!(sauce.acid, "applesauce is an acid food");
    assert_eq!(p.can_outcome(sauce, "boiling_water", 15.0, 0.0, false).state, PreserveState::Safe, "applesauce 15 minutes in boiling water");
    assert_eq!(p.can_outcome(sauce, "boiling_water", 10.0, 0.0, false).state, PreserveState::Spoils, "applesauce 10 minutes spoils, never botulism");
}

/// THE TESTED TABLES ARE THE GUIDE'S FIGURES (NCHFP, from the USDA guide, 2015; refutation pass
/// 2026-11-21), read at the bands the game uses and one above. A drift in the data from the source
/// fails here. Red with the carrots' pint time set to 20: "carrots, pints, weighted gauge, 0 ft:
/// 20 min, the guide says 25".
#[test]
fn the_tested_tables_are_the_guide_s_figures() {
    let p = shipped();
    let row = |recipe: &str, canner: &str| proc_of(&p, recipe).tested.iter().find(|t| t.canner == canner).cloned().unwrap_or_else(|| panic!("{recipe} has a {canner} row"));
    let v = |bands: &[Band], ft: f32| band_value(bands, ft).unwrap();
    let sauce = row("can_applesauce", "boiling_water");
    for (ft, pint, quart) in [(0.0, 15.0, 20.0), (2000.0, 20.0, 25.0), (5000.0, 20.0, 30.0), (7000.0, 25.0, 35.0)] {
        assert_eq!((v(&sauce.pints, ft), v(&sauce.quarts, ft)), (pint, quart), "applesauce, boiling water, {ft} ft");
    }
    let carrots = row("can_carrots", "pressure_weighted");
    assert_eq!(v(&carrots.pints, 0.0), 25.0, "carrots, pints, weighted gauge, 0 ft: {} min, the guide says 25", v(&carrots.pints, 0.0));
    assert_eq!((v(&carrots.quarts, 0.0), v(&carrots.psi, 0.0), v(&carrots.psi, 2000.0)), (30.0, 10.0, 15.0), "carrots quarts and pounds");
    let dial = row("can_carrots", "pressure_dial");
    assert_eq!([1000.0, 3000.0, 5000.0, 7000.0].map(|ft| v(&dial.psi, ft)), [11.0, 12.0, 13.0, 14.0], "carrots, dial gauge pounds");
    assert_eq!(band_value(&dial.psi, 9000.0), None, "no tested dial pressure above 8,000 ft");
    let potatoes = row("can_potatoes", "pressure_weighted");
    assert_eq!((v(&potatoes.pints, 0.0), v(&potatoes.quarts, 0.0)), (35.0, 40.0), "potatoes");
    assert!(proc_of(&p, "can_carrots").tested.iter().all(|t| t.canner != "boiling_water"), "no boiling-water process for carrots");
    let classes: Vec<(&str, f32)> = p.data.drying_classes.iter().map(|c| (c.id.as_str(), c.water_done)).collect();
    assert_eq!(classes, vec![("fruit", 0.20), ("vegetables", 0.10)], "NCHFP's dryness");
    let sun = p.method("sun").unwrap();
    assert_eq!((sun.min_air_c, sun.classes.clone()), (Some(29.4), vec!["fruit".to_string()]), "NCHFP's sun: 85 F, fruit only");
}

/// SIMPLIFIED ALWAYS COMES OUT SAFE: for every procedure the station's pick is safe. Red with the
/// station picking the tested minutes less one: "Simplified can_applesauce came out Spoils (TooShort)".
#[test]
fn simplified_always_comes_out_safe() {
    let p = shipped();
    for proc in &p.data.procedures {
        let pick = p.sensible(proc).unwrap_or_else(|| panic!("{} has a pick", proc.recipe));
        let out = p.outcome(proc, &pick, 20.0).unwrap();
        assert_eq!(out.state, PreserveState::Safe, "Simplified {} came out {:?} ({:?})", proc.recipe, out.state, out.reason);
    }
}

/// EVERY PRESERVING RECIPE IS DECIDED: each cooking-category recipe that trains preserving is a
/// procedure or listed in not_procedures with why, and every procedure names real items. Red with
/// can_potatoes' row removed: "can_potatoes trains preserving and is neither a procedure nor listed".
#[test]
fn every_preserving_recipe_is_decided() {
    let p = shipped();
    let recipes = RecipeRegistry::from_csv(&std::fs::read(data_dir().join("recipes.csv")).unwrap()).unwrap();
    let items = ItemRegistry::from_csv(&std::fs::read(data_dir().join("items.csv")).unwrap()).unwrap();
    for r in recipes.recipes.values() {
        if r.category == "cooking" && r.skill_required.as_deref() == Some("preserving") {
            let listed = p.data.not_procedures.iter().any(|(id, _)| *id == r.id);
            assert!(p.procedure(&r.id).is_some() || listed, "{} trains preserving and is neither a procedure nor listed", r.id);
        }
    }
    for proc in &p.data.procedures {
        let r = recipes.recipes.get(&proc.recipe).unwrap_or_else(|| panic!("{} is a recipe", proc.recipe));
        assert!(r.outputs.iter().any(|(id, _)| *id == proc.makes), "{} makes {}", proc.recipe, proc.makes);
        assert!(r.inputs.iter().any(|(id, _)| *id == proc.fresh), "{} starts from {}", proc.recipe, proc.fresh);
        assert!(items.items.contains_key(&proc.makes) && items.items.contains_key(&proc.fresh), "{}: real items", proc.recipe);
        if let Some(jar) = &proc.jar {
            assert!(r.inputs.iter().any(|(id, _)| id == jar), "{} uses the jar it hands back", proc.recipe);
        }
    }
}

/// WEIGHTS AND VOLUMES FOLLOW THE DATA: a drying recipe's output weighs what its input's solids
/// weigh at the dry water share (within one output item); a canned jar weighs its empty jar plus a
/// pint's food and takes the jar's room, and eating it hands the jar back. Red with dry_apples
/// making four bags: "dry_apples puts out 0.36 kg; its 1.00 kg of apples dry to 0.18 kg".
#[test]
fn weights_and_volumes_follow_the_data() {
    let p = shipped();
    let recipes = RecipeRegistry::from_csv(&std::fs::read(data_dir().join("recipes.csv")).unwrap()).unwrap();
    let items = ItemRegistry::from_csv(&std::fs::read(data_dir().join("items.csv")).unwrap()).unwrap();
    let kg = |id: &str| items.items.get(id).map(|d| d.mass_kg).unwrap_or_else(|| panic!("{id} in items.csv"));
    let litres = |id: &str| items.items.get(id).map(|d| d.volume_l).unwrap();
    for proc in &p.data.procedures {
        let r = &recipes.recipes[&proc.recipe];
        let out: f32 = r.outputs.iter().map(|(id, q)| kg(id) * *q as f32).sum();
        match proc.kind {
            Kind::Dry => {
                let w_in: f32 = r.inputs.iter().filter(|(id, _)| *id == proc.fresh).map(|(id, q)| kg(id) * *q as f32).sum();
                let done = p.class(proc.class.as_deref().unwrap()).unwrap().water_done;
                let want = dried_weight(w_in, proc.water_start, done);
                assert!((out - want).abs() <= kg(&proc.makes), "{} puts out {out:.2} kg; its {w_in:.2} kg of {} dry to {want:.2} kg", proc.recipe, proc.fresh);
            }
            Kind::Can => {
                let jar = proc.jar.as_deref().unwrap();
                let food = kg(&proc.makes) - kg(jar);
                assert!((0.3..=0.6).contains(&food), "{}: a pint's food weighs {food:.2} kg", proc.makes);
                assert!((litres(&proc.makes) - litres(jar)).abs() < 1e-6, "{} takes the jar's room", proc.makes);
                assert_eq!(p.empty_jar_of(&proc.makes), Some(jar), "eating {} hands back the jar", proc.makes);
            }
        }
    }
}

/// A preserved state round-trips through its byte and never reads as a cooked dish's.
#[test]
fn a_preserved_state_round_trips_through_its_byte() {
    for s in [PreserveState::Safe, PreserveState::Spoils, PreserveState::Botulism] {
        assert_eq!(PreserveState::from_byte(s.to_byte()), Some(s));
        assert!(crate::cooking::DishState::from_byte(s.to_byte()).is_none(), "{s:?}'s byte is no dish state");
    }
    assert_eq!(PreserveState::from_byte(0), None, "0 is a bought food");
}

// -- The systems the model reaches ----------------------------------------------------------------

fn food_store(seed: u64) -> DataStore {
    let mut data = DataStore::new();
    let reg = crate::status_effects::StatusEffectRegistry::from_csv(&std::fs::read(data_dir().join("status_effects.csv")).unwrap()).unwrap();
    data.insert("status_effect_registry", reg);
    data.insert("consume_request", std::sync::Mutex::new(Option::<String>::None));
    data.insert("drink_request", std::sync::Mutex::new(Option::<String>::None));
    data.insert("rest_request", std::sync::Mutex::new(false));
    data.insert("compost_request", std::sync::Mutex::new(false));
    data.insert("player_death", std::sync::Mutex::new(Option::<String>::None));
    data.insert(crate::sim_seed::SLOT, seed);
    data
}

fn body() -> Vitals {
    Vitals {
        satiation: 30.0,
        hydration: 80.0,
        energy: 100.0,
        oxygen: 100.0,
        body_temp_c: 37.0,
        waste: 0.0,
        satiation_max: 100.0,
        hydration_max: 100.0,
        energy_max: 100.0,
        oxygen_max: 100.0,
        waste_max: 100.0,
    }
}

/// Eat `n` of `item` in state `state`, each `age_h` hours old on its clock: (how many gave Food
/// Poisoning, how many gave Botulism, the empty jars handed back).
fn eat(item: &str, state: PreserveState, age_h: f64, n: u32) -> (u32, u32, u32) {
    let mut sys = crate::food::FoodSystem::new(data_dir());
    let data = food_store(7);
    let mut world = hecs::World::new();
    let mut inv = Inventory::new(24);
    inv.add_item_q(item, n, 99, state.to_byte());
    for s in inv.slots.iter_mut().flatten() {
        s.age_s = age_h * 3600.0;
    }
    let player = world.spawn((inv, body(), StatusEffects::default(), Health::default()));
    let (mut poisoned, mut botulism) = (0, 0);
    for _ in 0..n {
        *data.get::<std::sync::Mutex<Option<String>>>("consume_request").unwrap().lock().unwrap() = Some(item.to_string());
        sys.tick(&mut world, 0.0, &data);
        let mut fx = world.get::<&mut StatusEffects>(player).unwrap();
        if fx.has("food_poisoning") {
            poisoned += 1;
            fx.remove("food_poisoning");
        }
        if fx.has("botulism") {
            botulism += 1;
            fx.remove("botulism");
        }
        drop(fx);
        world.get::<&mut Vitals>(player).unwrap().satiation = 30.0;
    }
    let jars = world.get::<&Inventory>(player).unwrap().count_item("canning_jar_0");
    (poisoned, botulism, jars)
}

/// EACH METHOD CHANGES THE SPOILAGE CLOCK AS ITS DATA SAYS: dried apples 200 hours old are fresh
/// (their own 4,380 hours, NCHFP's six months at 80 F) while fresh apples keep 120; the same bag
/// taken off before it was dry spoils on the fresh apple's clock and poisons. A jar of carrots a
/// month old keeps (a year); one canned short of an acid food's time would keep only as long as the
/// fresh food. Red with food.rs reading the clock off the item alone: "under-dried apples 200 hours
/// old poisoned 0 of 4 (spoiled food always poisons)".
#[test]
fn each_method_changes_the_spoilage_clock_as_its_data_says() {
    let (ok, _, _) = eat("dried_apples_0", PreserveState::Safe, 200.0, 4);
    assert_eq!(ok, 0, "dried apples 200 hours old are still good");
    let (bad, _, _) = eat("dried_apples_0", PreserveState::Spoils, 200.0, 4);
    assert_eq!(bad, 4, "under-dried apples 200 hours old poisoned {bad} of 4 (spoiled food always poisons)");
    let (sauce_ok, _, _) = eat("applesauce_jar_0", PreserveState::Safe, 720.0, 2);
    assert_eq!(sauce_ok, 0, "a safe jar of applesauce a month old keeps");
    let (sauce_bad, _, _) = eat("applesauce_jar_0", PreserveState::Spoils, 720.0, 2);
    assert_eq!(sauce_bad, 2, "an under-processed jar a month old has spoiled");
}

/// BOTULISM: a botulism-risk jar makes the eater ill with Botulism on the data's chance; a safe jar
/// never does; every jar eaten hands back its empty jar. Red with food.rs never reading the byte for
/// botulism: "24 botulism-risk jars gave Botulism 0 times (the data's 0.5 expects about 12)".
#[test]
fn a_botulism_jar_gives_botulism_and_a_safe_one_never_does() {
    let (_, risky, jars) = eat("canned_carrots_0", PreserveState::Botulism, 1.0, 24);
    let risk = shipped().data.botulism_risk;
    assert!(risk > 0.0, "the data gives a chance");
    assert!((4..=20).contains(&risky), "24 botulism-risk jars gave Botulism {risky} times (the data's {risk} expects about 12)");
    assert_eq!(jars, 24, "every jar eaten hands back its empty jar");
    let (_, safe, _) = eat("canned_carrots_0", PreserveState::Safe, 1.0, 24);
    assert_eq!(safe, 0, "a jar pressure canned by the tested process never gives Botulism");
}

/// A hand craft of `recipe` on the real crafting system with the shipped tables, at no station, in
/// `mode` with the card's `pick`: (the stacks of what it makes, (count, byte)), the quest events).
fn craft(recipe: &str, inputs: &str, makes: &str, mode: Mode, pick: Option<Choice>) -> (Vec<(u32, u8)>, Vec<String>) {
    let csv = format!("id,name,category,inputs,outputs,craft_time_sec,station_required,skill_required,skill_level,description\n{recipe},Test,cooking,{inputs},{makes}:1,0,,preserving,0,test\n");
    let mut data = DataStore::new();
    data.insert("recipe_registry", RecipeRegistry::from_csv(csv.as_bytes()).unwrap());
    data.insert("item_registry", ItemRegistry::from_csv(&std::fs::read(data_dir().join("items.csv")).unwrap()).unwrap());
    data.insert("dev_stock_materials", std::sync::Mutex::new(false));
    data.insert("craft_request", std::sync::Mutex::new(Option::<String>::None));
    data.insert("quest_events", std::sync::Mutex::new(Vec::<String>::new()));
    crate::notices::register(&mut data);
    data.insert(DATA_KEY, shipped());
    data.insert(MODE_KEY, mode);
    data.insert(CHOICE_KEY, std::sync::Mutex::new(pick));
    let mut world = hecs::World::new();
    let mut inv = Inventory::new(16);
    for part in inputs.split('|') {
        let (id, q) = part.split_once(':').unwrap();
        inv.add_item(id, q.parse().unwrap(), 30);
    }
    let player = world.spawn((inv, Controllable));
    let mut sys = CraftingSystem::new();
    *data.get::<std::sync::Mutex<Option<String>>>("craft_request").unwrap().lock().unwrap() = Some(recipe.into());
    sys.tick(&mut world, 1.0, &data);
    let inv = world.get::<&Inventory>(player).unwrap();
    let stacks = inv.slots.iter().flatten().filter(|s| s.item_id == makes).map(|s| (s.quantity, s.quality)).collect();
    let events = data.get::<std::sync::Mutex<Vec<String>>>("quest_events").unwrap().lock().unwrap().clone();
    (stacks, events)
}

/// A CRAFTED JAR COMES OUT AS ITS METHOD SAYS: in Simplified the station's pick, safe, with a
/// Preserve event; in Realistic carrots the person puts in a boiling-water canner come out with a
/// botulism risk on the jar's byte and count for no Preserve step. Red with deliver_goods writing no
/// preserved state: "Simplified ignores the card: safe / left: [(1, 0)] / right: [(1, 65)]".
#[test]
fn a_crafted_jar_comes_out_as_its_method_says() {
    let (stacks, events) = craft("can_carrots", "vegetable_carrot_0:4|canning_jar_0:1", "canned_carrots_0", Mode::Simplified, Some(choice("can_carrots", "boiling_water", 90.0, 0.0, false)));
    assert_eq!(stacks, vec![(1, PreserveState::Safe.to_byte())], "Simplified ignores the card: safe");
    assert!(events.contains(&preserved_event_key("canned_carrots_0")), "a safe jar counts for a Preserve step: {events:?}");
    let (stacks, events) = craft("can_carrots", "vegetable_carrot_0:4|canning_jar_0:1", "canned_carrots_0", Mode::Realistic, Some(choice("can_carrots", "boiling_water", 90.0, 0.0, false)));
    let byte = stacks.first().map_or(0, |s| s.1);
    assert_eq!(byte, PreserveState::Botulism.to_byte(), "a water-bath jar of carrots came out with byte {byte}, not the botulism risk");
    assert!(!events.iter().any(|e| e.starts_with(PRESERVED_EVENT)), "a botulism jar counts for nothing: {events:?}");
}

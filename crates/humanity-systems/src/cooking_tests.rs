//! Cooking rung 1's tests (2026-11-21, docs/design/cooking.md): the model on the shipped data,
//! every stove and oven recipe decided, the dish state on a stack, and the systems it reaches:
//! a crafted dish that burns or comes out unsafe, and an eaten one's risk and vitamins.
//!
//! RED FIRST, 2026-11-21: with five faults put in on purpose, one run (`just test-lib cooking
//! --crate humanity-systems`, "7 failed") showed every test below red but the three data and byte
//! checks. The faults: cooking.rs's burn check ignoring `burns_above_c`, its beans' boil check
//! ignoring the medium, the station's pick rounding down with no margin; food.rs reading no dish
//! state off the eaten stack; crafting's burnt dish keeping its food. What each said is in its
//! comment.

use super::*;
use crate::crafting::{CraftingSystem, RecipeRegistry};
use crate::inventory::{Inventory, ItemRegistry};
use humanity_core::ecs::components::{Controllable, Health, StatusEffects, Vitals};
use humanity_core::ecs::systems::System;

fn data_dir() -> &'static Path {
    Path::new(concat!(env!("HUMANITY_REPO_ROOT"), "/data"))
}

fn shipped() -> Cooking {
    let c = Cooking::load(data_dir());
    assert!(!c.dishes.is_empty() && !c.classes.is_empty(), "food_system.ron's cooking tables load");
    c
}

fn dish<'a>(c: &'a Cooking, recipe: &str) -> &'a Dish {
    c.dish(recipe).unwrap_or_else(|| panic!("{recipe} is a dish"))
}

/// DONENESS FOLLOWS HEAT AND TIME, PER FOOD CLASS, on the shipped data: too short is
/// undercooked, the station's time is done, well past it overcooked, and only a medium hot enough
/// burns (a pan or an oven, never boiling water). Red with the station's pick rounded down:
/// "cook_fish: well past done it overcooks, got Done".
#[test]
fn doneness_follows_heat_and_time_per_food_class() {
    let c = shipped();
    for d in &c.dishes {
        let (heat, min) = c.sensible(d).unwrap_or_else(|| panic!("{}: no heat gets it done", d.recipe));
        let (_, class) = c.plan(d).unwrap();
        let short = c.outcome(d, heat, 1.0).unwrap();
        assert_eq!(short.doneness, Doneness::Undercooked, "{}: one minute at {} heat is undercooked", d.recipe, heat.id());
        let over = c.outcome(d, heat, min + class.overcook_after_min + 0.5).unwrap();
        assert!(matches!(over.doneness, Doneness::Overcooked | Doneness::Burnt), "{}: well past done it overcooks, got {:?}", d.recipe, over.doneness);
    }
    // Boiling water cannot burn an egg; a hot pan can.
    let eggs = dish(&c, "cook_boiled_eggs");
    assert_eq!(c.outcome(eggs, Heat::High, 60.0).unwrap().doneness, Doneness::Overcooked, "eggs boiled for an hour overcook and never burn: water cannot get hotter than the boil");
    let omelette = dish(&c, "cook_omelette");
    assert_eq!(c.outcome(omelette, Heat::High, 30.0).unwrap().doneness, Doneness::Burnt, "an omelette left 30 minutes in a hot pan burns");
    // A low oven never gets a roast to its safe temperature in a short time; the inside lags.
    let roast = dish(&c, "cook_roast_chicken");
    let early = c.outcome(roast, Heat::Low, 20.0).unwrap();
    assert!(early.doneness == Doneness::Undercooked && !early.safe, "a roast after 20 minutes in a low oven is undercooked and unsafe (inside {:.0} C)", early.core_c);
    assert!(early.core_c < 74.0, "the thermometer reads under the line: {:.0} C", early.core_c);
    // More heat gets there sooner (a pan; boiling water's medium is capped).
    let fish = dish(&c, "cook_fish");
    let lo = c.outcome(fish, Heat::Low, 1.0).unwrap().ready_min.unwrap();
    let hi = c.outcome(fish, Heat::High, 1.0).unwrap().ready_min.unwrap();
    assert!(hi < lo, "high heat grills fish sooner than low: {hi:.1} against {lo:.1} min");
}

/// BEANS: soft is not safe. FDA gives no safe line below the boil (its evidence is slow cookers at
/// 75 C or less), so the game counts beans simmered below a full boil as unsafe however soft: a
/// cautious game choice, not FDA's finding. Boiled long enough (FDA: at least 30 minutes) they are
/// done and safe. Red with the boil check ignoring the medium: "beans simmered 90 minutes below the
/// boil are soft but counted unsafe ...; got Outcome { doneness: Burnt, safe: true, ... }" (the
/// burn fault burnt them too).
#[test]
fn beans_simmered_go_soft_and_stay_unsafe_until_they_boil() {
    let c = shipped();
    let beans = dish(&c, "cook_beans");
    let simmered = c.outcome(beans, Heat::Low, 90.0).unwrap();
    assert!(
        matches!(simmered.doneness, Doneness::Done | Doneness::Overcooked) && !simmered.safe,
        "beans simmered 90 minutes below the boil are soft but counted unsafe (the game's cautious line: FDA gives none below the boil); got {simmered:?}"
    );
    let boiled = c.outcome(beans, Heat::Medium, 60.0).unwrap();
    assert!(boiled.doneness == Doneness::Done && boiled.safe, "beans boiled an hour are done and safe; got {boiled:?}");
    let quick = c.outcome(beans, Heat::Medium, 20.0).unwrap();
    assert!(!quick.safe, "20 minutes at the boil is short of FDA's 30");
}

/// SIMPLIFIED ALWAYS COMES OUT DONE: the station's pick makes every shipped dish done and safe.
/// Red with the pick rounded down and no margin: "cook_boiled_eggs at the station's pick (medium,
/// 10 min) came out Undercooked, safe false".
#[test]
fn simplified_always_comes_out_done_and_safe() {
    let c = shipped();
    for d in &c.dishes {
        let (heat, min) = c.sensible(d).unwrap();
        let out = c.outcome(d, heat, min).unwrap();
        assert!(
            out.doneness == Doneness::Done && out.safe,
            "{} at the station's pick ({}, {min} min) came out {:?}, safe {}",
            d.recipe,
            heat.id(),
            out.doneness,
            out.safe
        );
    }
}

/// EVERY STOVE AND OVEN RECIPE IS DECIDED, and every dish names real things: a cooking-category
/// recipe at the stove or the oven is a dish or a not-dish with why; a dish's recipe exists and
/// makes its `makes`, its method and class exist, its heat is a heat, and a class's done line is
/// at or past its safety line (so a done dish is a safe one).
#[test]
fn every_stove_and_oven_recipe_is_decided() {
    let c = shipped();
    let recipes = RecipeRegistry::from_csv(&std::fs::read(data_dir().join("recipes.csv")).unwrap()).unwrap();
    let mut bad = Vec::new();
    for r in recipes.recipes.values() {
        let at_heat = matches!(r.required_station.as_deref(), Some("stove_0") | Some("oven_0"));
        if r.category == "cooking" && at_heat && c.dish(&r.id).is_none() && !c.not_dishes.iter().any(|(id, _)| id == &r.id) {
            bad.push(format!("{} (at {:?}) is neither a dish nor a not-dish", r.id, r.required_station));
        }
    }
    for d in &c.dishes {
        match recipes.recipes.get(&d.recipe) {
            None => bad.push(format!("dish {} names no recipe", d.recipe)),
            Some(r) if !r.outputs.iter().any(|(id, _)| id == &d.makes) => bad.push(format!("dish {} does not make {}", d.recipe, d.makes)),
            _ => {}
        }
        if c.plan(d).is_none() {
            bad.push(format!("dish {}: method {} or class {} missing", d.recipe, d.method, d.class));
        }
        if Heat::from_id(&d.heat).is_none() {
            bad.push(format!("dish {}: heat {} is not low, medium or high", d.recipe, d.heat));
        }
        if !crate::food::consume_kinds().contains_key(&d.makes) {
            bad.push(format!("dish {}: {} is not food", d.recipe, d.makes));
        }
    }
    for (id, why) in &c.not_dishes {
        if !recipes.recipes.contains_key(id) || why.is_empty() {
            bad.push(format!("not-dish {id} names no recipe or gives no reason"));
        }
    }
    for k in &c.classes {
        if let Some(core) = k.safe.as_ref().and_then(|s| s.core_c) {
            if k.done_c < core {
                bad.push(format!("class {}: done at {} C, before its safety line {core} C", k.id, k.done_c));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// The dish state rides a stack's grade byte and comes back the same; 0 is "not cooked by hand".
#[test]
fn a_dish_state_round_trips_through_its_byte() {
    for doneness in [Doneness::Undercooked, Doneness::Done, Doneness::Overcooked] {
        for safe in [false, true] {
            for meal in 0..6u8 {
                let s = DishState { doneness, safe, meal };
                let b = s.to_byte().unwrap();
                assert!(b > 0);
                assert_eq!(DishState::from_byte(b), Some(s));
            }
        }
    }
    assert_eq!(DishState::from_byte(0), None);
    assert_eq!(DishState { doneness: Doneness::Burnt, safe: true, meal: 0 }.to_byte(), None, "a burnt dish is lost, never carried");
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

/// How many of `n` portions of `item` in state `state` made the eater ill.
fn bites(item: &str, state: DishState, n: u32) -> u32 {
    let mut sys = crate::food::FoodSystem::new(data_dir());
    let data = food_store(11);
    let mut world = hecs::World::new();
    let mut inv = Inventory::new(16);
    inv.add_item_q(item, n, 99, state.to_byte().unwrap());
    let player = world.spawn((inv, body(), StatusEffects::default(), Health::default()));
    let mut ill = 0;
    for _ in 0..n {
        *data.get::<std::sync::Mutex<Option<String>>>("consume_request").unwrap().lock().unwrap() = Some(item.to_string());
        sys.tick(&mut world, 1.0, &data);
        let mut fx = world.get::<&mut StatusEffects>(player).unwrap();
        if fx.has("food_poisoning") {
            ill += 1;
            fx.remove("food_poisoning");
        }
        drop(fx);
        world.get::<&mut Vitals>(player).unwrap().satiation = 30.0;
    }
    ill
}

/// UNDERCOOKED MEAT KEEPS ITS RISK AND COOKED THROUGH REMOVES IT: 48 portions of roast poultry
/// that never reached 74 C make the eater ill on raw meat's odds (0.40, food_system.ron raw_meat);
/// 48 cooked through make no one ill. Red with food.rs reading no dish state (the dish's own
/// profile, risk 0, was all it saw): "undercooked poultry made the eater ill 0 times in 48 (raw
/// meat's 0.40 expects about 19)".
#[test]
fn undercooked_meat_keeps_its_risk_and_cooked_through_removes_it() {
    let unsafe_ill = bites("roast_chicken_0", DishState { doneness: Doneness::Undercooked, safe: false, meal: 0 }, 48);
    assert!((8..=32).contains(&unsafe_ill), "undercooked poultry made the eater ill {unsafe_ill} times in 48 (raw meat's 0.40 expects about 19)");
    let done_ill = bites("roast_chicken_0", DishState { doneness: Doneness::Done, safe: true, meal: 2 }, 48);
    assert_eq!(done_ill, 0, "poultry cooked through never makes anyone ill");
}

/// THE NUTRITION CHANGE REACHES THE INTAKE LEDGER: an overcooked bowl of vegetable soup gives the
/// ledger its vitamin C and B group times the soup's USDA retention factors (code 3776, 0.85 and
/// 0.93, read from the data) of what a done bowl gives, and the same food energy. Red with food.rs
/// reading no dish state, when it still ate beans: "overcooked beans put 1.000 of a done bowl's
/// vitamin C in the ledger, not 0.65" (the refutation pass then gave beans USDA's own long-cooking
/// ratio, which keeps their vitamin C, so the soup carries the test).
#[test]
fn the_nutrition_cooking_changes_reaches_the_intake_ledger() {
    let took = |state: DishState| {
        let mut sys = crate::food::FoodSystem::new(data_dir());
        let data = food_store(3);
        let mut world = hecs::World::new();
        let mut inv = Inventory::new(16);
        inv.add_item_q("soup_vegetable_0", 1, 99, state.to_byte().unwrap());
        let player = world.spawn((inv, body(), StatusEffects::default(), Health::default(), crate::intake::IntakeLedger::default()));
        *data.get::<std::sync::Mutex<Option<String>>>("consume_request").unwrap().lock().unwrap() = Some("soup_vegetable_0".into());
        sys.tick(&mut world, 1.0, &data);
        let day = world.get::<&crate::intake::IntakeLedger>(player).unwrap().last_day(0.0);
        day
    };
    let done = took(DishState { doneness: Doneness::Done, safe: true, meal: 1 });
    let over = took(DishState { doneness: Doneness::Overcooked, safe: true, meal: 1 });
    assert!(done.kcal > 0.0 && done.vitamins_dv[2] > 0.0, "a done bowl reached the ledger: {done:?}");
    let k = shipped().keeps_of(dish(&shipped(), "cook_soup")).expect("the soup has retention factors");
    assert!(k.c < 1.0 && k.b < 1.0, "the soup's factors lose something: {k:?}");
    let c = over.vitamins_dv[2] / done.vitamins_dv[2];
    let b = over.vitamins_dv[1] / done.vitamins_dv[1];
    assert!((c - f64::from(k.c)).abs() < 1e-6, "overcooked soup put {c:.3} of a done bowl's vitamin C in the ledger, not {}", k.c);
    assert!((b - f64::from(k.b)).abs() < 1e-6, "and {b:.3} of its B group, not {}", k.b);
    assert!((over.kcal - done.kcal).abs() < 1e-9, "the food energy is the same: overcooking costs vitamins, not calories");
}

/// A hand craft of a dish, on the real crafting system with the shipped cooking tables: the
/// recipe at no station (the station gate is crafting's own, tested there), started with `choice`
/// in `mode`. Returns the backpack's egg_boiled_0 stacks (count, byte) and the quest events.
fn cook_eggs(mode: Mode, choice: Option<(Heat, f32)>) -> (Vec<(u32, u8)>, Vec<String>, u32) {
    let csv = "id,name,category,inputs,outputs,craft_time_sec,station_required,skill_required,skill_level,description\n\
               cook_boiled_eggs,Boil Eggs,cooking,egg_0:2,egg_boiled_0:2,0,,cooking,0,test\n";
    let mut data = DataStore::new();
    data.insert("recipe_registry", RecipeRegistry::from_csv(csv.as_bytes()).unwrap());
    data.insert("item_registry", ItemRegistry::from_csv(&std::fs::read(data_dir().join("items.csv")).unwrap()).unwrap());
    data.insert("dev_stock_materials", std::sync::Mutex::new(false));
    data.insert("craft_request", std::sync::Mutex::new(Option::<String>::None));
    data.insert("quest_events", std::sync::Mutex::new(Vec::<String>::new()));
    crate::notices::register(&mut data);
    data.insert(DATA_KEY, shipped());
    data.insert(MODE_KEY, mode);
    data.insert(CHOICE_KEY, std::sync::Mutex::new(choice.map(|(heat, minutes)| Choice { recipe: "cook_boiled_eggs".into(), heat, minutes })));
    let mut world = hecs::World::new();
    let mut inv = Inventory::new(16);
    inv.add_item("egg_0", 2, 30);
    let player = world.spawn((inv, Controllable));
    let mut sys = CraftingSystem::new();
    *data.get::<std::sync::Mutex<Option<String>>>("craft_request").unwrap().lock().unwrap() = Some("cook_boiled_eggs".into());
    sys.tick(&mut world, 1.0, &data);
    let inv = world.get::<&Inventory>(player).unwrap();
    let stacks = inv.slots.iter().flatten().filter(|s| s.item_id == "egg_boiled_0").map(|s| (s.quantity, s.quality)).collect();
    let eggs_left = inv.count_item("egg_0");
    let events = data.get::<std::sync::Mutex<Vec<String>>>("quest_events").unwrap().lock().unwrap().clone();
    (stacks, events, eggs_left)
}

/// A CRAFTED DISH COMES OUT AS ITS HEAT AND TIME SAY: in Simplified the station picks (done and safe,
/// one Cook event per egg, whatever the card held); in Guided the card's heat and time decide, and an
/// unsafe dish counts for no Cook step. Red with the station's pick rounded down: "Simplified comes
/// out done and safe: DishState { doneness: Undercooked, safe: false, meal: 0 }".
#[test]
fn a_crafted_dish_comes_out_as_its_heat_and_time_say() {
    // Simplified ignores the card: the station's pick, done and safe, a Cook event per egg.
    let (stacks, events, left) = cook_eggs(Mode::Simplified, Some((Heat::Low, 1.0)));
    assert_eq!(left, 0, "the eggs were used");
    let (n, byte) = stacks.first().copied().expect("boiled eggs made");
    assert_eq!(n, 2);
    let s = DishState::from_byte(byte).expect("a hand-cooked dish carries its state");
    assert!(s.doneness == Doneness::Done && s.safe, "Simplified comes out done and safe: {s:?}");
    assert_eq!(events.iter().filter(|e| *e == "cooked_egg_boiled_0").count(), 2, "two eggs cooked through: {events:?}");
    // Guided honours the card: one minute at low heat is undercooked and unsafe, and counts nothing.
    let (stacks, events, _) = cook_eggs(Mode::Guided, Some((Heat::Low, 1.0)));
    let s = DishState::from_byte(stacks[0].1).unwrap();
    assert!(s.doneness == Doneness::Undercooked && !s.safe, "one minute is undercooked and unsafe: {s:?}");
    assert!(!events.iter().any(|e| e.starts_with(COOKED_EVENT)), "an unsafe dish counts for no Cook step: {events:?}");
}

/// BURNING LOSES THE FOOD on the crafted path: an omelette-like egg dish fried far too long at
/// high heat leaves the eggs spent and nothing in the backpack. Red with deliver_goods keeping a
/// burnt dish's food: "a burnt dish still put 2 egg_boiled_0 in the backpack".
#[test]
fn burning_loses_the_food() {
    let mut c = shipped();
    // The boiled-egg recipe fried in a pan, so a burn is possible.
    c.dishes.iter_mut().find(|d| d.recipe == "cook_boiled_eggs").unwrap().method = "fried".into();
    let csv = "id,name,category,inputs,outputs,craft_time_sec,station_required,skill_required,skill_level,description\n\
               cook_boiled_eggs,Fry Eggs,cooking,egg_0:2,egg_boiled_0:2,0,,cooking,0,test\n";
    let mut data = DataStore::new();
    data.insert("recipe_registry", RecipeRegistry::from_csv(csv.as_bytes()).unwrap());
    data.insert("item_registry", ItemRegistry::from_csv(&std::fs::read(data_dir().join("items.csv")).unwrap()).unwrap());
    data.insert("dev_stock_materials", std::sync::Mutex::new(false));
    data.insert("craft_request", std::sync::Mutex::new(Option::<String>::None));
    data.insert("quest_events", std::sync::Mutex::new(Vec::<String>::new()));
    crate::notices::register(&mut data);
    data.insert(DATA_KEY, c);
    data.insert(MODE_KEY, Mode::Realistic);
    data.insert(CHOICE_KEY, std::sync::Mutex::new(Some(Choice { recipe: "cook_boiled_eggs".into(), heat: Heat::High, minutes: 40.0 })));
    let mut world = hecs::World::new();
    let mut inv = Inventory::new(16);
    inv.add_item("egg_0", 2, 30);
    let player = world.spawn((inv, Controllable));
    let mut sys = CraftingSystem::new();
    *data.get::<std::sync::Mutex<Option<String>>>("craft_request").unwrap().lock().unwrap() = Some("cook_boiled_eggs".into());
    sys.tick(&mut world, 1.0, &data);
    let inv = world.get::<&Inventory>(player).unwrap();
    assert_eq!(inv.count_item("egg_0"), 0, "the eggs were spent");
    assert_eq!(inv.count_item("egg_boiled_0"), 0, "a burnt dish still put {} egg_boiled_0 in the backpack", inv.count_item("egg_boiled_0"));
    let said = crate::notices::take(&data, crate::notices::REPLIES);
    assert!(said.iter().any(|l| l.contains("burnt")), "the cook is told it burnt: {said:?}");
    let events = data.get::<std::sync::Mutex<Vec<String>>>("quest_events").unwrap().lock().unwrap().clone();
    assert!(!events.iter().any(|e| e.starts_with("make_") || e.starts_with(COOKED_EVENT)), "nothing was made: {events:?}");
}

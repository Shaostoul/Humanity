//! Farm animals need care (2026-10-12, first-hour audit S6).
//!
//! Every test drives the care tick the way the game does, through the LivestockSystem and the
//! DataStore channels it reads (home storage, the tanks, the mode), against the shipped data
//! files, so the tests check that the game did what data/livestock/care.ron says.
//!
//! RED CHECKS. Each "Seen red" below was run on 2026-10-12 against the deliberate break it
//! names, on this change's own code, and the test then passed with the break taken out.

use super::*;
use crate::ecs::components::{Creature, Dead, Harvestable, Name, Transform, WaterTank};
use crate::ecs::systems::System;
use crate::systems::livestock::{CreatureRegistry, HerdSlot, LivestockSystem};
use glam::Vec3;
use std::path::Path;

fn data_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/data"))
}

fn shipped_care() -> CareData {
    CareData::from_ron(&std::fs::read(data_dir().join(FILE)).unwrap()).unwrap()
}

fn shipped_items() -> ItemRegistry {
    ItemRegistry::from_csv(&std::fs::read(data_dir().join("items.csv")).unwrap()).unwrap()
}

fn shipped_creatures() -> CreatureRegistry {
    CreatureRegistry::from_csv(&std::fs::read(data_dir().join("creatures.csv")).unwrap()).unwrap()
}

/// A DataStore as the game has it for the livestock tick: the shipped data, the channels,
/// home storage holding `stock`, and the mode.
fn store(stock: &[(&str, u32)], mode: Mode) -> DataStore {
    let mut data = DataStore::new();
    data.insert(DATA_KEY, shipped_care());
    data.insert("item_registry", shipped_items());
    data.insert("creature_registry", shipped_creatures());
    data.insert(
        "fluid_table",
        crate::systems::fluids::FluidTable::from_ron(&std::fs::read(data_dir().join("containers/fluids.ron")).unwrap()).unwrap(),
    );
    data.insert("home_stock", Mutex::new(stock.iter().map(|(k, v)| (k.to_string(), *v)).collect::<HashMap<String, u32>>()));
    data.insert("player_notices", Mutex::new(Vec::<String>::new()));
    data.insert(MODE_KEY, mode);
    crate::systems::livestock::register(&mut data);
    data
}

fn stock_of(data: &DataStore, id: &str) -> u32 {
    data.get::<Mutex<HashMap<String, u32>>>("home_stock").unwrap().lock().unwrap().get(id).copied().unwrap_or(0)
}

fn notices(data: &DataStore) -> Vec<String> {
    data.get::<Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap().clone()
}

/// One homestead animal of `species`, placed the way world entry places it.
fn herd_animal(world: &mut hecs::World, species: &str, slot: &str) -> hecs::Entity {
    let reg = shipped_creatures();
    let def = reg.get(species).unwrap();
    let p = def.renewable().unwrap();
    world.spawn((
        Creature { def_id: species.into(), anchor: Vec3::ZERO, range: 2.0, phase: 0.0, speed: 0.5, tint: [1.0; 3], body_side: 0.2 },
        Transform::default(),
        Name(def.name.clone()),
        Harvestable { resource: p.item, amount: p.amount as f32, regrow_time: p.regrow_s, time_since_harvest: 0.0 },
        HerdSlot(slot.into()),
    ))
}

fn tank(world: &mut hecs::World, litres: f32) -> hecs::Entity {
    world.spawn((WaterTank { liters: litres, capacity_l: 8000.0 },))
}

/// Run the livestock system for `hours` of game time in one-minute ticks (the game clock is
/// absent here, so the tick's dt is game seconds).
fn run(world: &mut hecs::World, data: &DataStore, hours: f64) {
    let mut sys = LivestockSystem::new();
    for _ in 0..(hours * 60.0).round() as u64 {
        sys.tick(world, 60.0, data);
    }
}

fn care(world: &hecs::World, e: hecs::Entity) -> AnimalCare {
    (*world.get::<&AnimalCare>(e).unwrap()).clone()
}

fn since(world: &hecs::World, e: hecs::Entity) -> f32 {
    world.get::<&Harvestable>(e).unwrap().time_since_harvest
}

/// THE SHIPPED DATA HOLDS TOGETHER. Every species with needs is a creatures.csv animal with a
/// yield; everything it eats is a feed with a dry-matter share and a real item with a mass
/// (a feed that is not an item would never be found in storage, and the animal would starve
/// beside a full barn); every day's amount is real; and the hours run in order, so an animal
/// slows before it stops, stops before it sickens, sickens before it dies.
///
/// Seen red with "hay_0" taken out of data/items.csv: "sheep eats hay_0, which is not an item
/// with a mass" (the goat or the sheep, whichever the map yields first).
#[test]
fn the_shipped_care_data_holds_together() {
    let cd = shipped_care();
    let items = shipped_items();
    let creatures = shipped_creatures();
    assert!(!cd.species.is_empty());
    for kept in ["chicken", "goat", "sheep"] {
        assert!(cd.species.contains_key(kept), "the homestead's {kept} has needs");
    }
    assert!(cd.forgiving_recovery > 0.0);
    for (id, sp) in &cd.species {
        let def = creatures.get(id).unwrap_or_else(|| panic!("{id} is not in creatures.csv"));
        assert!(def.renewable().is_some(), "{id} keeps for a yield");
        assert!(sp.dry_matter_kg_day > 0.0 && sp.water_l_day > 0.0, "{id} eats and drinks");
        assert!(!sp.eats.is_empty() && !sp.plural.is_empty(), "{id}");
        assert!(sp.recovery > 0.0, "{id} recovers");
        for feed in &sp.eats {
            assert!(cd.feeds.contains_key(feed), "{id} eats {feed}, which has no dry-matter share");
            assert!(cd.unit_dry_matter_kg(feed, Some(&items)).is_some(), "{id} eats {feed}, which is not an item with a mass");
        }
        for (what, l) in [("feed", sp.feed), ("water", sp.water)] {
            assert!(0.0 <= l.slows_h && l.slows_h < l.stops_h && l.stops_h <= l.sick_h && l.sick_h < l.dies_h, "{id} {what}: {l:?}");
        }
    }
    for share in cd.feeds.values() {
        assert!((0.5..=1.0).contains(share), "a dry-matter share of {share}");
    }
}

/// What the seeded home's Barn holds (data/places/seed.json), by item.
fn barn_stock() -> HashMap<String, u32> {
    fn find(v: &serde_json::Value) -> Option<&serde_json::Value> {
        if v.get("label").and_then(|l| l.as_str()) == Some("Barn") {
            return Some(v);
        }
        match v {
            serde_json::Value::Array(a) => a.iter().find_map(find),
            serde_json::Value::Object(o) => o.values().find_map(find),
            _ => None,
        }
    }
    let seed: serde_json::Value = serde_json::from_slice(&std::fs::read(data_dir().join("places/seed.json")).unwrap()).unwrap();
    let barn = find(&seed).expect("the seeded Barn");
    let mut out = HashMap::new();
    for c in barn["children"].as_array().unwrap() {
        if let (Some(id), Some(q)) = (c.get("item").and_then(|i| i.as_str()), c.get("qty").and_then(|q| q.as_u64())) {
            *out.entry(id.to_string()).or_insert(0) += q as u32;
        }
    }
    out
}

/// THE STARTING BARN FEEDS THE HERD FOR WEEKS, ON WHAT EACH EATS FIRST. The seeded home's Barn
/// must hold 30 days of every starter species' first feed (hay for the goats and sheep, grain
/// for the hens), or a new player's goats and sheep live on the grain meant for bread, and
/// then go hungry, before the player has learned where feed comes from.
///
/// Seen red with the 30 hay bales taken out of seed.json: "hay_0: the barn holds 0.0 kg of dry
/// matter, the herd eats 222.0 kg of it in 30 days".
#[test]
fn the_starting_barn_feeds_the_herd_for_weeks() {
    let cd = shipped_care();
    let items = shipped_items();
    let barn = barn_stock();
    let list = crate::systems::livestock::LivestockSpawnList::from_ron(&std::fs::read(data_dir().join("entities/livestock.ron")).unwrap()).unwrap();
    let mut demand: HashMap<String, f64> = HashMap::new();
    for p in &list.animals {
        let sp = cd.species.get(&p.creature).unwrap_or_else(|| panic!("{} has no care row", p.creature));
        *demand.entry(sp.eats[0].clone()).or_insert(0.0) += sp.dry_matter_kg_day * f64::from(p.count) * 30.0;
    }
    for (feed, need) in demand {
        let have = cd.unit_dry_matter_kg(&feed, Some(&items)).unwrap() * f64::from(barn.get(&feed).copied().unwrap_or(0));
        assert!(have >= need, "{feed}: the barn holds {have:.1} kg of dry matter, the herd eats {need:.1} kg of it in 30 days");
    }
}

/// A HEN EATS AND DRINKS HER DAY, FROM STORAGE AND THE TANKS, AND LAYS. One game day: about
/// 0.096 kg of dry matter, which is one 500 g bag of corn grain (0.435 kg of it) taken from
/// home storage; 0.25 L out of the tank; nothing short; and her egg 24 of its 26 hours along.
/// Corn is the first thing in her list, so the wheat is not touched.
///
/// Seen red with `eat` never taking from storage (the trough topped up with nothing): "a bag of
/// corn out of storage: left 10".
#[test]
fn a_hen_eats_and_drinks_her_day_from_storage_and_the_tanks() {
    let data = store(&[("grain_corn_0", 10), ("grain_wheat_0", 10)], Mode::Realistic);
    let mut world = hecs::World::new();
    let water = tank(&mut world, 100.0);
    let hen = herd_animal(&mut world, "chicken", "chicken#0");
    run(&mut world, &data, 24.0);
    assert_eq!(stock_of(&data, "grain_corn_0"), 9, "a bag of corn out of storage: left {}", stock_of(&data, "grain_corn_0"));
    assert_eq!(stock_of(&data, "grain_wheat_0"), 10, "corn first");
    let c = care(&world, hen);
    assert!((c.trough_kg - (0.435 - 0.096)).abs() < 1e-6, "the rest of the bag waits in her trough: {}", c.trough_kg);
    let left = world.get::<&WaterTank>(water).unwrap().liters;
    let drank = f64::from(100.0 - left) - c.drinker_l;
    assert!((drank - 0.25).abs() < 0.001, "a day's water out of the tank: {drank}");
    assert_eq!((c.hungry_h, c.thirsty_h), (0.0, 0.0));
    assert!((since(&world, hen) - 86_400.0).abs() < 1.0, "a day of her 26-hour egg: {}", since(&world, hen));
    assert!(notices(&data).is_empty(), "nothing to tell: {:?}", notices(&data));
    run(&mut world, &data, 2.0);
    assert!(since(&world, hen) >= 93_600.0, "her egg is laid");
}

/// A HEN WITH NO FEED LAYS LESS, THEN NOTHING, AND IN REALISTIC FALLS SICK AND DIES. With water
/// but no feed: full rate for her first day short (slows at 24 hours), half rate at 60, none
/// from 96; sick at 10 days; dead of starvation at 21, her slot on the lost list so she is not
/// spawned again. The player is told once that the chickens have nothing to eat (naming what
/// they eat), once that she is sick, and that she died.
///
/// Seen red with `Limits::production` returning 1 whatever the hours ("half rate at 60 h:
/// 1"), and separately with the death check taken out ("dead at 21 days").
#[test]
fn a_hen_without_feed_lays_less_then_nothing_then_dies_in_realistic() {
    let data = store(&[], Mode::Realistic);
    let mut world = hecs::World::new();
    tank(&mut world, 1000.0);
    let hen = herd_animal(&mut world, "chicken", "chicken#2");
    let sp = shipped_care().species["chicken"].clone();
    let rate = |w: &hecs::World| care(w, hen).production(&sp, Mode::Realistic);
    run(&mut world, &data, 23.9);
    assert_eq!(rate(&world), 1.0, "a day short: still laying");
    run(&mut world, &data, 0.1);
    run(&mut world, &data, 36.0);
    assert!((rate(&world) - 0.5).abs() < 0.02, "half rate at 60 h: {}", rate(&world));
    run(&mut world, &data, 36.1);
    assert_eq!(rate(&world), 0.0, "no eggs from 4 days");
    let stalled = since(&world, hen);
    run(&mut world, &data, 24.0);
    assert_eq!(since(&world, hen), stalled, "her egg does not grow while she lays nothing");
    let told = notices(&data);
    assert_eq!(told.iter().filter(|n| n.contains("chickens have nothing to eat")).count(), 1, "{told:?}");
    assert!(told.iter().any(|n| n.contains("Corn Grain")), "names what they eat: {told:?}");
    run(&mut world, &data, 241.0 - 120.0);
    assert!(care(&world, hen).sick(&sp, Mode::Realistic), "sick at 10 days");
    assert!(notices(&data).iter().any(|n| n.contains("sick")), "{:?}", notices(&data));
    assert!(world.get::<&Dead>(hen).is_err());
    run(&mut world, &data, 505.0 - 241.0);
    let dead = world.get::<&Dead>(hen).map(|d| d.cause.clone());
    assert_eq!(dead.as_deref(), Ok("starved"), "dead at 21 days");
    assert!(notices(&data).iter().any(|n| n == "A chicken has starved to death"), "{:?}", notices(&data));
    run(&mut world, &data, 0.1);
    assert_eq!(lost(&data), vec![LostAnimal { slot: "chicken#2".into(), cause: "starved".into() }]);
    assert!(is_lost(&data, "chicken#2"));
}

/// IN FORGIVING, NEGLECT ONLY STOPS PRODUCTION. Six weeks with no feed and no water: she stops
/// laying and is very hungry and very thirsty, but never sick and never dead; fed and watered
/// again she comes back as fast as she fell off (`forgiving_recovery`), laying at full rate a
/// day after her water and 4 days after her feed came back.
///
/// Seen red with Forgiving's cap taken out: "held where laying stops", left (1008.0, 1008.0)
/// hours short, right (96.0, 24.0). (Forgiving never kills whether or not the count is capped;
/// the cap is what lets her come back.)
#[test]
fn in_forgiving_neglect_only_stops_production() {
    let data = store(&[], Mode::Forgiving);
    let mut world = hecs::World::new();
    let hen = herd_animal(&mut world, "chicken", "chicken#0");
    let sp = shipped_care().species["chicken"].clone();
    run(&mut world, &data, 42.0 * 24.0);
    let dead = world.get::<&Dead>(hen).map(|d| d.cause.clone());
    assert!(dead.is_err(), "never dead in Forgiving: {dead:?}");
    let c = care(&world, hen);
    assert!(!c.sick(&sp, Mode::Forgiving));
    assert_eq!((c.hungry_h, c.thirsty_h), (sp.feed.stops_h, sp.water.stops_h), "held where laying stops");
    assert_eq!(c.production(&sp, Mode::Forgiving), 0.0);
    assert_eq!(condition_words(&c, &sp, Mode::Forgiving), "Very hungry and very thirsty (no feed in home storage, the water tanks are empty)");
    assert!(lost(&data).is_empty());

    // Feed and water come back.
    data.get::<Mutex<HashMap<String, u32>>>("home_stock").unwrap().lock().unwrap().insert("grain_corn_0".into(), 50);
    tank(&mut world, 500.0);
    run(&mut world, &data, sp.feed.stops_h);
    let c = care(&world, hen);
    assert_eq!(c.production(&sp, Mode::Forgiving), 1.0, "laying again: {c:?}");
    assert_eq!(condition_words(&c, &sp, Mode::Forgiving), "Fed and watered");
}

/// WATER COMES FROM THE TANKS, THEN STORED TAP WATER; WITH NEITHER, THE ANIMAL GOES THIRSTY.
/// A goat (8.4 L a day) with a 2 L tank drinks it dry, then the purified-water jugs in storage
/// (the fluid table's tap water), one litre at a time; with those gone she goes short, and the
/// player is told once that the goats have no water. A goat with hay eats hay before grain.
///
/// Seen red with `stored_water` returning None (no jugs drunk: "drank the 3 jugs: left 3").
#[test]
fn water_comes_from_the_tanks_then_stored_tap_water() {
    let data = store(&[("water_purified_0", 3), ("hay_0", 1), ("grain_oat_0", 5)], Mode::Realistic);
    let mut world = hecs::World::new();
    let water = tank(&mut world, 2.0);
    let goat = herd_animal(&mut world, "goat", "goat#0");
    run(&mut world, &data, 16.0);
    assert_eq!(world.get::<&WaterTank>(water).unwrap().liters, 0.0, "the tank first");
    assert_eq!(stock_of(&data, "water_purified_0"), 0, "drank the 3 jugs: left {}", stock_of(&data, "water_purified_0"));
    assert!(care(&world, goat).thirsty_h > 0.0, "and then went short");
    assert_eq!(notices(&data).iter().filter(|n| n.contains("goats have no water")).count(), 1, "{:?}", notices(&data));
    assert_eq!((stock_of(&data, "hay_0"), stock_of(&data, "grain_oat_0")), (0, 5), "hay before grain");
}

/// THE YIELD TIMER KEEPS COUNTING, A FRAME AT A TIME, ALL YEAR. A sheep's wool takes 365 days.
/// The game ticks once a frame, and in the component's f32 a frame's 1/60 s is lost outright
/// once the count passes about 4 days (half the spacing between f32 values there is more than
/// a frame), so a fleece counted that way would never be ready. The care tick counts in f64 and
/// writes the f32. A year of minute ticks, fed and watered, brings her to the last hour; that
/// hour at 60 frames a second makes the fleece ready; and a collect (the E bridge setting the
/// timer to 0) starts the next one, counted by frames too.
///
/// Seen red with the care tick adding each frame in f32 (`h.time_since_harvest + dt * share`):
/// "the last hour at 60 frames a second: 31532400" (not a second of it counted).
#[test]
fn a_sheeps_year_of_wool_counts_frame_by_frame() {
    let data = store(&[("hay_0", 40)], Mode::Realistic);
    let mut world = hecs::World::new();
    tank(&mut world, 8000.0);
    let sheep = herd_animal(&mut world, "sheep", "sheep#0");
    run(&mut world, &data, 365.0 * 24.0 - 1.0);
    assert_eq!(since(&world, sheep), 31_532_400.0, "a year less an hour, by minutes");
    let mut sys = LivestockSystem::new();
    let frames = |world: &mut hecs::World, n: u32, sys: &mut LivestockSystem| {
        for _ in 0..n {
            sys.tick(world, 1.0 / 60.0, &data);
        }
    };
    frames(&mut world, 3600 * 60, &mut sys);
    let s = since(&world, sheep);
    assert!(s >= 31_536_000.0, "the last hour at 60 frames a second: {s}");
    {
        let mut h = world.get::<&mut Harvestable>(sheep).unwrap();
        assert_eq!(crate::systems::livestock::collect(&mut h), Some(8), "8 x 0.4 kg of wool");
    }
    frames(&mut world, 600 * 60, &mut sys);
    assert!((since(&world, sheep) - 600.0).abs() < 0.1, "the next fleece starts: {}", since(&world, sheep));
}

/// CARE AND THE DEAD SURVIVE A SAVE AND A RESTORE. The herd's care goes into the save by slot
/// and through JSON; a new launch's herd, spawned after the save was applied, takes it back,
/// and the time away is fed out of storage without making anyone hungry while there is feed.
/// A slot that died is not on the new herd: a herd already in the world (a character select)
/// loses it at once.
///
/// Seen red with `attach` ignoring the waiting save (every animal fresh): "the hungry goat comes
/// back hungry: 0.016666666666666666" (one minute short, from scratch).
#[test]
fn care_and_the_dead_survive_a_save_and_a_restore() {
    let data = store(&[], Mode::Realistic);
    let mut world = hecs::World::new();
    tank(&mut world, 100.0);
    let goat = herd_animal(&mut world, "goat", "goat#0");
    herd_animal(&mut world, "goat", "goat#1");
    run(&mut world, &data, 30.0); // no feed: hungry
    let hungry = care(&world, goat).hungry_h;
    assert!(hungry > 25.0, "{hungry}");
    let saved = herd_care(&world);
    let json = serde_json::to_string(&saved).unwrap();
    let saved: Vec<CareSave> = serde_json::from_str(&json).unwrap();
    assert_eq!(saved.len(), 2);
    let dead = vec![LostAnimal { slot: "goat#1".into(), cause: "died of thirst".into() }];

    // A new launch: the save is applied before world entry, with 2 days away and hay in the barn.
    let data = store(&[("hay_0", 10)], Mode::Realistic);
    let mut world = hecs::World::new();
    restore(&mut world, &data, saved.clone(), dead.clone(), 2.0 * DAY_S);
    assert_eq!(pending_care(&data), Some(saved.clone()), "waits for the herd");
    assert!(is_lost(&data, "goat#1"));
    tank(&mut world, 100.0);
    let goat = herd_animal(&mut world, "goat", "goat#0");
    run(&mut world, &data, 1.0 / 60.0);
    let c = care(&world, goat);
    assert!((c.hungry_h - hungry).abs() < 0.1, "the hungry goat comes back hungry: {}", c.hungry_h);
    assert_eq!(stock_of(&data, "hay_0"), 9, "2 days of 2.1 kg fed out of storage: one 17.2 kg bale");
    assert!(notices(&data).iter().any(|n| n.starts_with("While you were away your animals ate 1 Hay")), "{:?}", notices(&data));
    assert_eq!(pending_care(&data), None, "taken");

    // A character select: the herd is in the world, and the save says goat#1 died.
    let mut world = hecs::World::new();
    herd_animal(&mut world, "goat", "goat#0");
    let other = herd_animal(&mut world, "goat", "goat#1");
    restore(&mut world, &data, saved, dead, 0.0);
    assert!(!world.contains(other), "the dead goat is not in the herd");
}

/// THE TIME AWAY FEEDS THE ANIMALS OUT OF STORAGE, AND WHEN IT RUNS OUT THEY COME BACK HUNGRY,
/// BUT NEVER SICK. A hen away 30 days with one bag of corn: she eats it (4.5 days) and goes
/// short for the rest, held where laying stops (4 days short), not sick, not dead.
///
/// Seen red with the away cap taken out (`care.hungry_h + short_h`): "held where laying stops:
/// 611".
#[test]
fn the_time_away_feeds_from_storage_and_never_sickens() {
    let data = store(&[("grain_corn_0", 1)], Mode::Realistic);
    let mut world = hecs::World::new();
    restore(&mut world, &data, vec![], vec![], 30.0 * DAY_S);
    tank(&mut world, 100.0);
    let hen = herd_animal(&mut world, "chicken", "chicken#0");
    run(&mut world, &data, 1.0 / 60.0);
    let sp = shipped_care().species["chicken"].clone();
    let c = care(&world, hen);
    assert_eq!(stock_of(&data, "grain_corn_0"), 0);
    assert!((c.hungry_h - sp.feed.stops_h).abs() < 0.1, "held where laying stops: {:.0}", c.hungry_h);
    assert!(!c.sick(&sp, Mode::Realistic) && world.get::<&Dead>(hen).is_err());
    assert!(notices(&data).iter().any(|n| n.contains("ran out: 1 animal went hungry")), "{:?}", notices(&data));
}

/// WHAT THE PLAYER READS. The walk-up lines for a fed hen and a hungry one, a forage plant
/// (no care: the old prompt), and the Home page's rows, the dead included.
///
/// Seen red with `condition_words` always saying "Fed and watered": "Fed and watered (no feed
/// in home storage). Eats about 110 g of Corn Grain ..." for the hungry hen.
#[test]
fn the_walk_up_lines_and_the_animals_card_say_it_plainly() {
    let data = store(&[("grain_corn_0", 5)], Mode::Realistic);
    let mut world = hecs::World::new();
    tank(&mut world, 100.0);
    let hen = herd_animal(&mut world, "chicken", "chicken#0");
    run(&mut world, &data, 8.0);
    let (prompt, line) = walk_up_lines(&world, hen, &data);
    assert_eq!(prompt, "Chicken: Egg in 18 h");
    assert_eq!(line, "Fed and watered. Eats about 110 g of Corn Grain and drinks about 0.25 L of water a day.");

    let data = store(&[], Mode::Realistic);
    let mut world = hecs::World::new();
    tank(&mut world, 100.0);
    let hen = herd_animal(&mut world, "chicken", "chicken#0");
    run(&mut world, &data, 72.0);
    let (prompt, _) = walk_up_lines(&world, hen, &data);
    assert_eq!(prompt, "[E] collect Egg (Chicken)", "she laid while she still could");
    let laid = herd_rows(&world, &data);
    assert!(laid[0].ready && laid[0].product == "Egg ready to collect", "{:?}", laid[0]);
    crate::systems::livestock::collect(&mut world.get::<&mut Harvestable>(hen).unwrap());
    run(&mut world, &data, 1.0 / 60.0);
    let (prompt, line) = walk_up_lines(&world, hen, &data);
    assert!(prompt.starts_with("Chicken: Egg in about") && prompt.ends_with("at its slower rate"), "{prompt}");
    assert!(line.starts_with("Hungry (no feed in home storage). Eats about 110 g of Corn Grain"), "{line}");
    data.get::<Mutex<Vec<LostAnimal>>>(LOST_KEY).unwrap().lock().unwrap().push(LostAnimal { slot: "chicken#1".into(), cause: "starved".into() });
    let rows = herd_rows(&world, &data);
    assert_eq!(rows.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(), ["Chicken 1", "Chicken 2"]);
    assert!(rows[0].alert && rows[0].status.starts_with("Hungry"), "{:?}", rows[0]);
    assert_eq!(rows[1].status, "Starved to death.");
    // The same facts apart, for the Home page's table (2026-10-30): each column its own part of
    // the lines above, so the table and the cards cannot say different things.
    let hen = &rows[0];
    assert_eq!((hen.kind.as_str(), hen.feed.as_str(), hen.water.as_str()), ("Chicken", "110 g of Corn Grain", "0.25 L"), "{hen:?}");
    assert!(hen.condition.starts_with("Hungry (no feed in home storage)") && !hen.dead && !hen.ready, "{hen:?}");
    assert!(hen.product.starts_with("Egg in about") && hen.status == format!("{}. {}.", hen.condition, hen.product), "{hen:?}");
    assert_eq!(hen.needs, format!("Eats about {} and drinks about {} of water a day.", hen.feed, hen.water));
    let lost = &rows[1];
    assert!(lost.dead && lost.alert && lost.condition == "Starved to death" && lost.kind == "Chicken" && lost.feed.is_empty(), "{lost:?}");

    // A berry bush has a yield and no needs: the prompt it always had, and no care line.
    let bush = crate::systems::livestock::spawn_creature_at(&mut world, shipped_creatures().get("berry_bush").unwrap(), Some(&shipped_items()), Vec3::ZERO, [1.0; 3]);
    let (prompt, line) = walk_up_lines(&world, bush, &data);
    assert!(prompt.starts_with("[E] collect"), "{prompt}");
    assert!(line.is_empty());
}

/// The Settings hint's numbers are the chicken's own, read from the data.
///
/// Seen red with the hint reading the water's sick hours where it means its stop hours: "A
/// chicken without water stops laying after 2 days, falls sick after 2 days ...".
#[test]
fn the_settings_hint_reads_the_data() {
    let hint = hint_text(&shipped_care());
    assert!(hint.contains("stops laying after a day, falls sick after 2 days and dies after 3 days"), "{hint}");
    assert!(hint.contains("never fall sick or die"), "{hint}");
}

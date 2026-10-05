//! The automated machines through the time away (crafting::away). Each test
//! was seen red by breaking the rule it pins, as its doc comment says.

use std::collections::HashMap;
use std::sync::Mutex;

use super::away::{self, AwayWork};
use super::{CraftSave, CraftingSystem, RecipeRegistry};
use crate::ecs::components::{AutoRefine, Controllable, MachineInstanceId, MachineType, PowerConsumer, StationLoad};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::inventory::{Inventory, ItemRegistry};

const HEADER: &str = "id,name,category,inputs,outputs,craft_time_sec,station_required,skill_required,skill_level,description\n";
const ITEMS: &str = "id,name,weight_kg,stack_size,volume_l\n\
    grain_0,Grain,0.5,99,0.9\nflour_0,Flour,0.5,99,0.9\nore_0,Ore,1,99,0.3\ningot_0,Ingot,1,99,0.1\n\
    plank_0,Plank,1,99,2\nhammer_0,Hammer,0.8,99,0.3\n";

/// A home with the given recipes, home storage and an empty backpack.
fn home(recipes: &str, stock: &[(&str, u32)]) -> (DataStore, hecs::World, hecs::Entity) {
    let mut data = DataStore::new();
    data.insert("recipe_registry", RecipeRegistry::from_csv(format!("{HEADER}{recipes}").as_bytes()).unwrap());
    data.insert("item_registry", ItemRegistry::from_csv(ITEMS.as_bytes()).unwrap());
    data.insert("player_notices", Mutex::new(Vec::<String>::new()));
    let stock: HashMap<String, u32> = stock.iter().map(|(id, q)| (id.to_string(), *q)).collect();
    data.insert("home_stock", Mutex::new(stock));
    data.insert("home_stock_outputs", Mutex::new(Vec::<(String, u32)>::new()));
    super::register(&mut data);
    let mut world = hecs::World::new();
    let player = world.spawn((Inventory::new(16), Controllable));
    (data, world, player)
}

fn machine(world: &mut hecs::World, id: &str, recipe: &str, keep: Option<u32>) -> hecs::Entity {
    world.spawn((
        AutoRefine { recipe_id: recipe.to_string(), keep },
        MachineInstanceId(id.to_string()),
        MachineType(id.to_string()),
    ))
}

fn away_for(secs: f64) -> AwayWork {
    AwayWork { secs, power_balance_w: [0.0; 2], ..Default::default() }
}

/// What was filed for home storage, by item.
fn filed(data: &DataStore, id: &str) -> u32 {
    data.get::<Mutex<Vec<(String, u32)>>>("home_stock_outputs")
        .unwrap()
        .lock()
        .unwrap()
        .iter()
        .filter(|(i, _)| i == id)
        .map(|(_, q)| *q)
        .sum()
}

fn stored(data: &DataStore, id: &str) -> u32 {
    data.get::<Mutex<HashMap<String, u32>>>("home_stock").unwrap().lock().unwrap().get(id).copied().unwrap_or(0)
}

fn notices(data: &DataStore) -> Vec<String> {
    data.get::<Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap().clone()
}

/// The grain mill mills while the player is away, on the grain the Barn
/// holds, until it has its 20 flour on hand, and then rests, exactly as it
/// would at home: 7 batches of 3, 28 of the 40 grain. One tick, no session
/// time. Seen red with `away::take` returning None (0 flour), and with
/// `Ledger::collect_filed` a no-op (30 flour: the mill never saw its own).
#[test]
fn the_mill_catches_up_to_its_keep_target_and_rests() {
    let (data, mut world, _player) = home("mill,Mill,refining,grain_0:4,flour_0:3,10,,,0,test\n", &[("grain_0", 40)]);
    machine(&mut world, "mill_1", "mill", Some(20));
    away::hand_over(&data, Some(away_for(3600.0)));
    let mut sys = CraftingSystem::new();
    sys.tick(&mut world, 0.0, &data);
    assert_eq!(filed(&data, "flour_0"), 21, "milled to the keep target: {:?}", notices(&data));
    assert_eq!(stored(&data, "grain_0"), 12, "the rest of the grain stays whole");
    assert!(sys.active_crafts.is_empty(), "resting, not starting another batch on unfiled flour");
    assert!(notices(&data).iter().any(|n| n == "While you were away, the home's machines made 21 Flour."), "{:?}", notices(&data));
    assert!(away::take(&data).is_none(), "taken once");
}

/// A week away makes no more than the stock could supply: home storage's 5
/// ore make two ingots and its last 1 stays, nothing more however long the
/// absence, and the 3 ore in the backpack are left alone (BUG-150: a machine
/// takes from the store it is fed from, never from the player's pockets).
/// Seen red by skipping the input check in `away_start` (it made ingots from
/// nothing); and, for the backpack, before BUG-150's fix: "the backpack is
/// left alone" (left: 0, right: 3), when the backpack was spent first.
#[test]
fn a_long_absence_makes_only_what_the_stock_could_supply() {
    let (data, mut world, player) = home("smelt,Smelt,refining,ore_0:2,ingot_0:1,30,,,0,test\n", &[("ore_0", 5)]);
    world.get::<&mut Inventory>(player).unwrap().add_item("ore_0", 3, 99);
    machine(&mut world, "smelter_1", "smelt", None);
    away::hand_over(&data, Some(away_for(7.0 * 86_400.0)));
    let mut sys = CraftingSystem::new();
    sys.tick(&mut world, 0.0, &data);
    assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("ore_0"), 3, "the backpack is left alone");
    assert_eq!(filed(&data, "ingot_0"), 2);
    assert_eq!(stored(&data, "ore_0"), 1, "the odd one stays in home storage");
    assert!(sys.active_crafts.is_empty());
}

/// The smelter's ingots feed the workbench within the same time away, and
/// only the hammers are filed: the ingots made and used in between are not
/// filed as well (no double count). Seen red by not moving each delivery
/// into the pool (`Ledger::collect_filed` a no-op): 0 hammers.
#[test]
fn one_machine_feeds_the_next_through_the_time_away() {
    let (data, mut world, _player) = home(
        "smelt,Smelt,refining,ore_0:2,ingot_0:1,30,,,0,test\n\
         hammer,Hammer,crafting,ingot_0:1|plank_0:1,hammer_0:1,20,,,0,test\n",
        &[("ore_0", 4), ("plank_0", 5)],
    );
    machine(&mut world, "smelter_1", "smelt", None);
    machine(&mut world, "bench_1", "hammer", None);
    away::hand_over(&data, Some(away_for(3600.0)));
    CraftingSystem::new().tick(&mut world, 0.0, &data);
    assert_eq!(filed(&data, "hammer_0"), 2);
    assert_eq!(filed(&data, "ingot_0"), 0, "used as it was made");
    assert_eq!(stored(&data, "plank_0"), 3);
}

/// A smelter resting at its keep target of one ingot goes back to work the
/// moment the workbench takes that ingot, as it would on the next frame at
/// home, not when the workbench next finishes: two hammers in a minute.
/// Seen red with the start pass run once per moment (1 hammer).
#[test]
fn a_machine_resting_at_its_keep_target_resumes_when_its_product_is_taken() {
    let (data, mut world, _player) = home(
        "smelt,Smelt,refining,ore_0:2,ingot_0:1,30,,,0,test\n\
         hammer,Hammer,crafting,ingot_0:1|plank_0:1,hammer_0:1,20,,,0,test\n",
        &[("ore_0", 20), ("plank_0", 5), ("ingot_0", 1)],
    );
    machine(&mut world, "smelter_1", "smelt", Some(1));
    machine(&mut world, "bench_1", "hammer", None);
    away::hand_over(&data, Some(away_for(60.0)));
    let mut sys = CraftingSystem::new();
    sys.tick(&mut world, 0.0, &data);
    assert_eq!(filed(&data, "hammer_0"), 2);
    assert_eq!(sys.active_crafts.len(), 2, "both at work when the player returns");
}

/// Drone ore is in home storage only from the moment it landed: ore landing
/// 5 s before the end of an hour starts one 10 s batch, which the player
/// finds still running (5 s to go), not two finished ingots. Since BUG-150
/// the drone unloads into home storage (`mining::deliver_haul` files the
/// haul), so when the machines run the haul is either already put away in
/// the Barn or still on the channel waiting to be; both are covered. Seen
/// red with the hauls left out of the ledger (2 ingots, made from the
/// start); and, once the haul moved to home storage, with the ore not yet
/// landed subtracted from the backpack instead of from home storage:
/// "nothing finished before the time ran out (put away: true)" (left: 2,
/// right: 0).
#[test]
fn drone_ore_is_used_only_from_the_moment_it_landed() {
    for put_away in [true, false] {
        let stock: &[(&str, u32)] = if put_away { &[("ore_0", 4)] } else { &[] };
        let (data, mut world, player) = home("smelt,Smelt,refining,ore_0:2,ingot_0:1,10,,,0,test\n", stock);
        if !put_away {
            data.get::<Mutex<Vec<(String, u32)>>>("home_stock_outputs").unwrap().lock().unwrap().push(("ore_0".into(), 4));
        }
        machine(&mut world, "smelter_1", "smelt", None);
        let mut work = away_for(3600.0);
        work.hauls = vec![(3595.0, vec![("ore_0".to_string(), 4)])];
        away::hand_over(&data, Some(work));
        let mut sys = CraftingSystem::new();
        sys.tick(&mut world, 0.0, &data);
        assert_eq!(filed(&data, "ingot_0"), 0, "nothing finished before the time ran out (put away: {put_away})");
        assert_eq!(sys.active_crafts.len(), 1, "one batch in flight at login (put away: {put_away})");
        assert!((sys.active_crafts[0].time_remaining - 5.0).abs() < 1e-3, "{}", sys.active_crafts[0].time_remaining);
        assert_eq!(sys.active_crafts[0].machine_id.as_deref(), Some("smelter_1"));
        assert_eq!(
            stored(&data, "ore_0") + filed(&data, "ore_0"),
            2,
            "its inputs spent, the rest waits in home storage (put away: {put_away})"
        );
        assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("ore_0"), 0, "never in the backpack");
    }
}

fn electric_mill(world: &mut hecs::World) {
    let e = machine(world, "mill_1", "mill", None);
    world
        .insert(
            e,
            (
                PowerConsumer { draw_watts: 0.0, priority: 3, enabled: true },
                StationLoad { active_watts: 1000.0, idle_watts: 0.0 },
            ),
        )
        .unwrap();
}

/// An electric machine draws only the power the home could spare, no faster
/// than the home made it: 100 W spare for two hours pays for two 100 Wh
/// batches, the first at the one-hour mark and the second at the end, which
/// the player finds running. With the stock and the time for 20. Seen red
/// with the machine treated as off the grid (`electric: false`).
#[test]
fn an_electric_machine_runs_only_on_power_the_home_could_spare() {
    let (data, mut world, _player) = home("mill,Mill,refining,grain_0:1,flour_0:1,360,,,0,test\n", &[("grain_0", 99)]);
    electric_mill(&mut world);
    let mut work = away_for(7200.0);
    work.power_balance_w = [100.0, 100.0];
    away::hand_over(&data, Some(work));
    let mut sys = CraftingSystem::new();
    sys.tick(&mut world, 0.0, &data);
    assert_eq!(filed(&data, "flour_0"), 1, "one finished");
    assert_eq!(sys.active_crafts.len(), 1, "the second started at the end, on power made by then");
    assert_eq!(stored(&data, "grain_0"), 97);
}

/// A home that makes less power than it uses has none to spare: the
/// electric mill makes nothing, and the player is told why instead of
/// finding an idle mill with no word. Seen red the same way.
#[test]
fn a_home_short_of_power_runs_no_electric_machine_and_says_so() {
    let (data, mut world, _player) = home("mill,Mill,refining,grain_0:1,flour_0:1,360,,,0,test\n", &[("grain_0", 99)]);
    electric_mill(&mut world);
    let mut work = away_for(7200.0);
    work.power_balance_w = [-500.0, -500.0];
    away::hand_over(&data, Some(work));
    let mut sys = CraftingSystem::new();
    sys.tick(&mut world, 0.0, &data);
    assert_eq!(filed(&data, "flour_0"), 0);
    assert!(sys.active_crafts.is_empty());
    assert_eq!(stored(&data, "grain_0"), 99, "nothing spent");
    assert!(
        notices(&data).iter().any(|n| n.starts_with("The mill 1 did not run while you were away")),
        "{:?}",
        notices(&data)
    );
}

/// The batch a machine had in flight at the save finished 5 s into the time
/// away: it lands then and the machine carries on from that moment, 9 more
/// batches of 10 s in the 100 s, and the tenth is running at login. Seen red
/// without the take-over (the machine counted as busy: only the restored
/// batch, through the session).
#[test]
fn the_batch_in_flight_at_the_save_lands_and_the_machine_carries_on() {
    let (data, mut world, _player) = home("mill,Mill,refining,grain_0:1,flour_0:1,10,,,0,test\n", &[("grain_0", 99)]);
    machine(&mut world, "mill_1", "mill", None);
    // As resume_home hands them over: the batch counted down by the time
    // away (restored_crafts), and when it was due.
    *data.get::<Mutex<Option<Vec<CraftSave>>>>("restore_active_crafts").unwrap().lock().unwrap() =
        Some(vec![CraftSave { recipe_id: "mill".into(), time_remaining: 0.0, auto: true, machine_id: Some("mill_1".into()), pad: None }]);
    let mut work = away_for(100.0);
    work.busy.insert("mill_1".into(), 5.0);
    away::hand_over(&data, Some(work));
    let mut sys = CraftingSystem::new();
    sys.tick(&mut world, 0.0, &data);
    assert_eq!(filed(&data, "flour_0"), 10, "the saved batch and nine more");
    assert_eq!(sys.active_crafts.len(), 1);
    assert!((sys.active_crafts[0].time_remaining - 5.0).abs() < 1e-3);
    assert_eq!(stored(&data, "grain_0"), 89, "ten batches started while away, the saved one's grain spent before");
}

/// Nothing runs until the machines exist (world entry, or the menu's power
/// spawn): the hand-over waits for them rather than being spent on nothing.
#[test]
fn the_time_away_waits_for_the_machines() {
    let (data, mut world, _player) = home("mill,Mill,refining,grain_0:4,flour_0:3,10,,,0,test\n", &[("grain_0", 8)]);
    away::hand_over(&data, Some(away_for(3600.0)));
    let mut sys = CraftingSystem::new();
    sys.tick(&mut world, 0.0, &data);
    assert_eq!(filed(&data, "flour_0"), 0);
    machine(&mut world, "mill_1", "mill", None);
    sys.tick(&mut world, 0.0, &data);
    assert_eq!(filed(&data, "flour_0"), 6);
}

/// The whole of the power an electric machine may draw away is the Usage
/// meter's day balance: the shipped homes make less than they use in both
/// life support modes (the 2026-09-27 meter), so neither may run one.
#[test]
fn the_shipped_homes_have_no_power_to_spare_while_away() {
    for file in ["home.ron", "home_solo.ron"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines").join(file);
        let home = crate::machines::MachineHome::load(&path).expect("parses");
        let [station, realistic] = away::day_power_balance(&home);
        assert!(
            station < 0.0 && realistic < station,
            "{file}: {station} W, {realistic} W; if a home now makes power to spare, update docs/design/offline-progression.md (Power)"
        );
    }
}

#[test]
fn the_notice_names_what_was_made() {
    let items = ItemRegistry::from_csv(ITEMS.as_bytes()).unwrap();
    let report = away::AwayReport {
        batches: 3,
        made: vec![("flour_0".into(), 21), ("hammer_0".into(), 2)],
        unpowered: vec!["grain_mill".into()],
    };
    assert_eq!(
        away::notice(&report, Some(&items)).unwrap(),
        "While you were away, the home's machines made 21 Flour and 2 Hammer. \
         The grain mill did not run while you were away: the home makes no more power than it uses, so it had none to spare."
    );
    assert_eq!(away::notice(&away::AwayReport::default(), Some(&items)), None);
    assert_eq!(away::join_list(&["a".into(), "b".into(), "c".into()]), "a, b and c");
}

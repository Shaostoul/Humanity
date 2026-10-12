//! The mining drone's running costs (first-hour audit S5, 2026-10-12): its energy from
//! the home's grid, its wear and service, the two modes, and the time away. Each test
//! names the deliberate break it was seen failing against.

use super::*;
use crate::ecs::components::Battery;
use crate::systems::electrical::ElectricalSystem;
use std::collections::HashMap;
use std::sync::Mutex;

type Order = Option<(String, Vec<(String, u32)>)>;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// The drone as shipped (data/mining/drone.ron).
fn shipped() -> DroneSpec {
    DroneSpec::from_ron(include_str!("../../data/mining/drone.ron")).expect("the shipped drone data parses")
}

/// A DataStore with the drone's data registered, in Realistic or Simplified, holding `unit`.
fn store(spec: DroneSpec, realistic: bool, unit: DroneUnit) -> DataStore {
    let mut data = DataStore::new();
    let items = ItemRegistry::from_csv(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/items.csv"))).expect("items.csv");
    data.insert("item_registry", items);
    data.insert("commission_drone", Mutex::new(Order::None));
    data.insert("auto_mine_order", Mutex::new(Order::None));
    data.insert("player_notices", Mutex::new(Vec::<String>::new()));
    data.insert(UNIT_KEY, Mutex::new(unit));
    data.insert(SPEC_KEY, spec);
    data.insert(REQUEST_KEY, Mutex::new(Option::<DroneRequest>::None));
    data.insert(READOUT_KEY, Mutex::new(Option::<DroneReadout>::None));
    data.insert(MODE_KEY, Mutex::new(realistic));
    data
}

/// A drone with an empty battery.
fn flat(spec: &DroneSpec) -> DroneUnit {
    DroneUnit { pack_wh: 0.0, ..DroneUnit::new(spec) }
}

/// One of the home's battery banks (data/machines/home.ron's figures) on island 0.
fn bank(world: &mut hecs::World, charge_wh: f32) -> hecs::Entity {
    world.spawn((
        Battery { charge_wh, capacity_wh: 4000.0, max_charge_w: 2000.0, max_discharge_w: 2000.0 },
        PowerCircuit { island: 0 },
    ))
}

fn rock(iron: f32) -> AsteroidBody {
    AsteroidBody {
        id: "rock".into(),
        name: "Asteroid R-1".into(),
        classification: "M".into(),
        ores: vec![("iron_ore_0".into(), iron)],
        position: [0.0, 0.0, 0.0],
    }
}

fn commission(data: &DataStore, units: u32) {
    *data.get::<Mutex<Order>>("commission_drone").unwrap().lock().unwrap() =
        Some(("rock".into(), vec![("iron_ore_0".into(), units)]));
}

fn keep_mining(data: &DataStore, units: u32) {
    *data.get::<Mutex<Order>>("auto_mine_order").unwrap().lock().unwrap() =
        Some(("rock".into(), vec![("iron_ore_0".into(), units)]));
}

fn notices(data: &DataStore) -> Vec<String> {
    data.get::<Mutex<Vec<String>>>("player_notices").unwrap().lock().unwrap().clone()
}

fn unit_now(data: &DataStore) -> DroneUnit {
    unit(data).expect("the drone's state is registered")
}

fn shown(data: &DataStore) -> DroneReadout {
    readout(data).expect("the drone system showed the panel something")
}

fn flying(world: &hecs::World) -> usize {
    world.query::<&Drone>().iter().count()
}

fn banks_wh(world: &hecs::World) -> f32 {
    world.query::<&Battery>().iter().map(|(_, b)| b.charge_wh).sum()
}

fn iron(world: &hecs::World, player: hecs::Entity) -> u32 {
    world.get::<&Inventory>(player).unwrap().count_item("iron_ore_0")
}

/// The game's order: the ElectricalSystem ticks before the DroneSystem.
struct Home {
    electrical: ElectricalSystem,
    drones: DroneSystem,
}

impl Home {
    fn new() -> Self {
        Home { electrical: ElectricalSystem::new(&data_dir()), drones: DroneSystem::new() }
    }
    fn step(&mut self, world: &mut hecs::World, data: &DataStore, dt: f32) {
        self.electrical.tick(world, dt, data);
        self.drones.tick(world, dt, data);
    }
}

/// A TRIP'S ENERGY COMES OUT OF THE HOME. A flat drone at its dock charges from the
/// home's batteries through the electrical sim (two banks, 4 kW of discharge, the
/// dock's 3 kW): 4,264 Wh at 3 kW is 85 minutes, and the banks give up what the
/// battery took. Then it takes off, the trip takes the whole charge, and it wears one
/// trip.
///
/// Seen red 2026-10-12 with `set_charger_draw` asking the grid for 0 W always: "the
/// dock charged it from the home: 0 Wh" (the battery never charged).
#[test]
fn a_trip_takes_its_charge_from_the_home_batteries() {
    let spec = shipped();
    let data = store(spec.clone(), true, flat(&spec));
    let mut world = hecs::World::new();
    world.spawn((Inventory::new(16), Controllable));
    world.spawn((rock(100.0),));
    bank(&mut world, 4000.0);
    bank(&mut world, 4000.0);
    let mut home = Home::new();
    for _ in 0..600 {
        home.step(&mut world, &data, 10.0);
    }
    let u = unit_now(&data);
    assert!(u.charged(&spec), "the dock charged it from the home: {} Wh", u.pack_wh);
    let paid = 8000.0 - banks_wh(&world);
    assert!((paid - spec.trip_wh).abs() < 40.0, "the home's batteries paid for the charge: {paid} Wh");
    assert!(shown(&data).power_line.contains("Battery full"), "{}", shown(&data).power_line);

    commission(&data, 10);
    home.step(&mut world, &data, 1.0);
    assert_eq!(flying(&world), 1, "a charged drone takes off");
    let u = unit_now(&data);
    assert!(u.pack_wh < 1.0, "the trip took the whole charge: {} Wh left", u.pack_wh);
    assert_eq!(u.wear_trips, 1.0, "and wore it one trip");
}

/// REALISTIC: A FLAT HOME KEEPS THE DRONE DOCKED, AND SAYS WHY. The banks are empty and
/// nothing generates, so the grid sheds the dock, the battery gets nothing, and a Launch
/// is refused with the reason; the panel says it too. Two hours later it is still
/// docked.
///
/// Seen red 2026-10-12 with `commission_with_costs` skipping its charge check: "a drone
/// without a trip's charge stays docked" (left: 1, right: 0).
#[test]
fn realistic_a_flat_home_keeps_the_drone_docked_and_says_why() {
    let spec = shipped();
    let data = store(spec.clone(), true, flat(&spec));
    let mut world = hecs::World::new();
    world.spawn((Inventory::new(16), Controllable));
    world.spawn((rock(100.0),));
    bank(&mut world, 0.0);
    let mut home = Home::new();
    for _ in 0..3 {
        home.step(&mut world, &data, 1.0);
    }
    commission(&data, 10);
    home.step(&mut world, &data, 1.0);
    assert_eq!(flying(&world), 0, "a drone without a trip's charge stays docked");
    let said = notices(&data);
    assert!(
        said.iter().any(|n| n.contains("cannot take off yet") && n.contains("no power to spare")),
        "the player is told why: {said:?}"
    );
    let r = shown(&data);
    assert!(r.power_line.starts_with("Not charging: the home has no power to spare"), "{}", r.power_line);
    assert_eq!(r.blocked.as_deref(), Some("Its battery holds 0.0 kWh of the 4.3 kWh a trip needs."));
    for _ in 0..720 {
        home.step(&mut world, &data, 10.0);
    }
    assert_eq!(flying(&world), 0, "still docked two hours on");
    assert_eq!(unit_now(&data).pack_wh, 0.0, "and still flat");
}

/// SIMPLIFIED: A FLAT HOME ONLY DELAYS THE TRIP. The same flat home: the Launch waits
/// for the battery, the dock charges it anyway at its 3 kW, and the drone takes off by
/// itself 85 minutes later, wearing half a trip. The home's empty banks gave nothing.
///
/// Seen red 2026-10-12 with `charge_at_dock` charging nothing when the grid shed the
/// dock in Simplified too: "it took off by itself once charged" (left: 0, right: 1).
#[test]
fn simplified_a_flat_home_only_delays_the_trip() {
    let spec = shipped();
    let data = store(spec.clone(), false, flat(&spec));
    let mut world = hecs::World::new();
    world.spawn((Inventory::new(16), Controllable));
    world.spawn((rock(100.0),));
    bank(&mut world, 0.0);
    let mut home = Home::new();
    for _ in 0..3 {
        home.step(&mut world, &data, 1.0);
    }
    commission(&data, 10);
    home.step(&mut world, &data, 1.0);
    assert_eq!(flying(&world), 0, "not yet");
    assert!(unit_now(&data).held.is_some(), "the Launch waits for the battery");
    let said = notices(&data);
    assert!(said.iter().any(|n| n.contains("takes off when its battery holds a trip's 4.3 kWh")), "{said:?}");
    assert_eq!(shown(&data).held_target.as_deref(), Some("Asteroid R-1"));
    // 4,264 Wh at 3 kW: 5,117 s.
    let mut waited = 0.0;
    while flying(&world) == 0 && waited < 6000.0 {
        home.step(&mut world, &data, 10.0);
        waited += 10.0;
    }
    assert_eq!(flying(&world), 1, "it took off by itself once charged");
    assert!((5110.0..5140.0).contains(&waited), "after a trip's charge at 3 kW: {waited} s");
    let u = unit_now(&data);
    assert_eq!((u.wear_trips, u.held.is_none()), (0.5, true), "half a trip's wear, nothing waiting");
    assert_eq!(banks_wh(&world), 0.0, "the empty banks gave nothing");
}

/// WEAR: A FULL TRIP EACH IN REALISTIC, HALF IN SIMPLIFIED, AND A DRONE DUE FOR ITS
/// SERVICE STAYS DOCKED. With a two-trip service interval (and a charger quick enough
/// that the test need not wait 85 minutes a trip), Keep mining flies two trips in
/// Realistic and four in Simplified, then the drone stays home: told once, and the
/// panel says why.
///
/// Seen red 2026-10-12 with `pay_for_trip` adding no wear: "two trips, then it stays
/// docked" (left: 140, right: 20: it kept flying).
#[test]
fn the_drone_wears_per_trip_and_stays_docked_when_due_for_its_service() {
    for (realistic, trips) in [(true, 2u32), (false, 4u32)] {
        let mut spec = shipped();
        spec.service_trips = 2.0;
        spec.charger_w = 3_600_000.0; // 1,000 Wh a second
        let data = store(spec.clone(), realistic, DroneUnit::new(&spec));
        keep_mining(&data, 10);
        let mut world = hecs::World::new();
        let player = world.spawn((Inventory::new(16), Controllable));
        world.spawn((rock(10_000.0),));
        bank(&mut world, 4000.0); // the dock needs a home grid; no ElectricalSystem here, so it is always fed
        let mut sys = DroneSystem::new();
        for _ in 0..200 {
            sys.tick(&mut world, 1.0, &data);
        }
        assert_eq!(iron(&world, player), trips * 10, "{trips} trips, then it stays docked (realistic: {realistic})");
        assert_eq!(flying(&world), 0);
        let u = unit_now(&data);
        assert_eq!(u.wear_trips, 2.0, "worn to its service");
        let told = notices(&data).iter().filter(|n| n.contains("due for its service")).count();
        assert_eq!(told, 1, "told once");
        assert_eq!(shown(&data).blocked.as_deref(), Some("Due for its service: it stays docked until it is repaired."));
        assert_eq!(shown(&data).trips_left, 0);
    }
}

/// A SERVICE TAKES ITS PARTS, THE BACKPACK FIRST AND THEN HOME STORAGE, AND ONLY WHEN
/// EVERY PART IS THERE. Missing the rubber: nothing is taken and the player is told
/// what is missing. With it: one carbon fibre sheet from the backpack, the other and the
/// rubber from home storage, and the wear is gone.
///
/// Seen red 2026-10-12 with `service` not taking from home storage: "the rest from home
/// storage" (left: 5, right: 4).
#[test]
fn a_service_takes_its_parts_from_the_backpack_then_home_storage() {
    let spec = shipped();
    let worn = DroneUnit { wear_trips: spec.service_trips, told_worn: true, ..DroneUnit::new(&spec) };
    let mut data = store(spec.clone(), true, worn);
    let stock: HashMap<String, u32> = [("carbon_fiber_sheet_0".to_string(), 5)].into_iter().collect();
    data.insert("home_stock", Mutex::new(stock));
    let mut world = hecs::World::new();
    let mut pack = Inventory::new(16);
    pack.add_item("carbon_fiber_sheet_0", 1, 10);
    let player = world.spawn((pack, Controllable));
    let mut sys = DroneSystem::new();
    let home = |data: &DataStore, id: &str| data.get::<Mutex<HashMap<String, u32>>>("home_stock").unwrap().lock().unwrap().get(id).copied().unwrap_or(0);

    request(&data, DroneRequest::Repair);
    sys.tick(&mut world, 1.0, &data);
    assert_eq!(unit_now(&data).wear_trips, spec.service_trips, "no service without the rubber");
    assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("carbon_fiber_sheet_0"), 1, "nothing taken");
    assert_eq!(home(&data, "carbon_fiber_sheet_0"), 5, "nothing taken");
    assert!(notices(&data).iter().any(|n| n.contains("needs 1 Rubber Sheet more")), "{:?}", notices(&data));
    assert!(!shown(&data).can_service, "the panel's Repair is off");

    data.get::<Mutex<HashMap<String, u32>>>("home_stock").unwrap().lock().unwrap().insert("rubber_sheet_0".into(), 1);
    sys.tick(&mut world, 1.0, &data);
    let r = shown(&data);
    assert!(r.can_service, "every part is on hand now");
    assert_eq!(r.service_parts.iter().map(|p| (p.need, p.have)).collect::<Vec<_>>(), vec![(2, 6), (1, 1)]);

    request(&data, DroneRequest::Repair);
    sys.tick(&mut world, 1.0, &data);
    let u = unit_now(&data);
    assert_eq!((u.wear_trips, u.told_worn), (0.0, false), "serviced");
    assert_eq!(world.get::<&Inventory>(player).unwrap().count_item("carbon_fiber_sheet_0"), 0, "the backpack's first");
    assert_eq!(home(&data, "carbon_fiber_sheet_0"), 4, "the rest from home storage");
    assert_eq!(home(&data, "rubber_sheet_0"), 0);
    assert!(notices(&data).iter().any(|n| n.contains("The drone is serviced")), "{:?}", notices(&data));
}

/// THE SHIPPED DATA FITS THE GAME. The real drone carries every hold the game can give
/// it (10 units of the heaviest ore in items.csv), the data folder's copy is the one
/// built in, and every service part is an item a recipe makes.
///
/// Seen red 2026-10-12 with payload_kg set to 65 in data/mining/drone.ron (the 149.9 kg
/// band's figure): "the drone lifts the heaviest hold: 80 kg of platinum_ore_0 > 65".
#[test]
fn the_shipped_drone_data_carries_every_hold_and_its_parts_can_be_made() {
    let spec = shipped();
    assert_eq!(DroneSpec::load(&data_dir()), Some(spec.clone()), "the data folder's copy loads");
    let items_csv = include_str!("../../data/items.csv");
    let (ore, kg) = items_csv
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            (c.len() > 5 && c[3] == "ore").then(|| (c[0].to_string(), c[5].parse::<f32>().unwrap_or(0.0)))
        })
        .fold((String::new(), 0.0_f32), |a, b| if b.1 > a.1 { b } else { a });
    let hold = kg * DRONE_CAPACITY as f32;
    assert!(hold > 0.0, "the ore rows were read");
    assert!(hold <= spec.payload_kg, "the drone lifts the heaviest hold: {hold} kg of {ore} > {}", spec.payload_kg);
    let items = ItemRegistry::from_csv(items_csv.as_bytes()).unwrap();
    let recipes = crate::systems::crafting::RecipeRegistry::from_csv(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/recipes.csv"
    )))
    .unwrap();
    for (id, _) in &spec.service_parts {
        assert!(items.items.contains_key(id), "{id} is an item");
        assert!(recipes.recipes.values().any(|r| r.outputs.iter().any(|(o, _)| o == id)), "{id} can be made");
    }
}

/// THE TIME AWAY CHARGES ON THE POWER THE HOME COULD SPARE, AND THAT PACES THE TRIPS.
/// Eight hours away with Keep mining, the home sparing 1.5 kW (half the dock's 3 kW): a
/// trip's charge takes 10,234 s, so a full drone flies three trips and is charging the
/// fourth when the player comes back, and the home gave it two trips' charge and the
/// rest. With nothing to spare only the trip already charged flies; in Simplified the
/// dock charges at its own rate anyway and the home pays nothing.
///
/// Seen red 2026-10-12 with `advance_away` launching without charging (the old free
/// drone): "three trips in eight hours" (left: 100, right: 3).
#[test]
fn away_the_dock_charges_on_the_power_the_home_could_spare() {
    let spec = shipped();
    let data = store(spec.clone(), true, DroneUnit::new(&spec));
    keep_mining(&data, 10);
    let mut world = hecs::World::new();
    world.spawn((Inventory::new(16), Controllable));
    world.spawn((rock(1000.0),));
    let away = advance_away(&mut world, &data, 8.0 * 3600.0, 1500.0);
    assert_eq!(away.hauls.len(), 3, "three trips in eight hours");
    let wait = f64::from(spec.trip_wh) * 3600.0 / 1500.0;
    let times: Vec<f64> = away.hauls.iter().map(|(t, _)| *t).collect();
    for (got, want) in times.iter().zip([9.0, 9.0 + wait + 9.0, 9.0 + 2.0 * (wait + 9.0)]) {
        assert!((got - want).abs() < 0.01, "home at {times:?}");
    }
    let rest = 8.0 * 3600.0 - times[2];
    let u = unit_now(&data);
    assert!((f64::from(u.pack_wh) - rest * 1500.0 / 3600.0).abs() < 1.0, "charging the fourth: {} Wh", u.pack_wh);
    assert_eq!(u.wear_trips, 3.0);
    let drawn_want = 2.0 * f64::from(spec.trip_wh) + rest * 1500.0 / 3600.0;
    assert!((away.drawn_wh - drawn_want).abs() < 1.0, "the home gave {} Wh, not {drawn_want}", away.drawn_wh);

    // Nothing to spare: the trip already charged, and no more.
    let data = store(spec.clone(), true, DroneUnit::new(&spec));
    keep_mining(&data, 10);
    let mut world = hecs::World::new();
    world.spawn((Inventory::new(16), Controllable));
    world.spawn((rock(1000.0),));
    let away = advance_away(&mut world, &data, 8.0 * 3600.0, 0.0);
    assert_eq!((away.hauls.len(), away.drawn_wh), (1, 0.0), "only the trip already charged");

    // Simplified: the dock's own 3 kW, paid by nobody.
    let data = store(spec.clone(), false, DroneUnit::new(&spec));
    keep_mining(&data, 10);
    let mut world = hecs::World::new();
    world.spawn((Inventory::new(16), Controllable));
    world.spawn((rock(1000.0),));
    let away = advance_away(&mut world, &data, 8.0 * 3600.0, 0.0);
    assert_eq!((away.hauls.len(), away.drawn_wh), (6, 0.0), "a 5,117 s charge and a 9 s trip each, paid by nobody");
}

/// The Settings hint reads its numbers from the data it describes. Seen red 2026-10-12
/// with `hint_text` given no data: "the hint names a trip's energy".
#[test]
fn the_mode_hint_reads_its_numbers_from_the_data() {
    let spec = shipped();
    let hint = hint_text(Some(&spec));
    assert!(hint.contains("4.3 kWh"), "the hint names a trip's energy: {hint}");
    assert!(hint.contains("3.0 kW") && hint.contains("86 min"), "{hint}");
    assert!(hint.contains("Every 100 trips") && hint.contains("half as fast (200 trips a service)"), "{hint}");
}

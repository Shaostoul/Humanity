//! The farming golden state (docs/design/sim-frame-cost.md, "Proof first", 2026-10-30): a varied
//! garden run for a fixed number of ticks, and the exact state it ends in. FarmingSystem's speed
//! work (F1 per-tick lookup tables, F2 no allocations per crop) must leave every bit of that state
//! the same; this test is how that is shown rather than argued.
//!
//! What it checks, in order:
//! 1. The run is DETERMINISTIC. The tick keeps several HashMaps (`breathed`, `tally`, `short_now`,
//!    `ph_out`), and each map's order is seeded at random; a float summed across keys in that order
//!    would end differently from one run to the next and make any golden meaningless. So the same
//!    garden is built and run twice, each with its own freshly seeded maps, and must end identical.
//! 2. The garden actually did something: crops moved past their first stage, pests arrived, the
//!    rooms' air was tracked. A golden of an inert garden would prove nothing about the loops F1
//!    and F2 rewrite.
//! 3. The end state's BLAKE3 matches the recorded golden in `golden/farming_state.txt`. The state is
//!    canonical JSON: every crop with its soil, in entity order, and every SoilMemory, with every
//!    map's keys sorted (SoilMemory is nested HashMaps), so the bytes never depend on map order.
//!
//! When a change is MEANT to alter the garden's behaviour, rerun with HUMANITY_WRITE_GOLDEN=1 and
//! say in the commit why the state moved. A speed change must never need that.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use super::gardening_tests::{make_store, set_garden_speed};
use super::humidity::{self, GrowRoom, HumidityData, HUMIDITY_RON};
use super::lighting::GrowPlot;
use super::*;
use crate::ecs::components::{
    AirHandler, CropInstance, CropSoil, Irrigator, PlumbingCircuit, PowerConsumer, SoilMemory, Transform, WaterConsumer,
    WaterProducer, WaterTank,
};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::atmosphere::{EnclosedSpace, HomeAir};
use crate::systems::life_support::{self, LifeSupportData, LIFE_SUPPORT_RON};
use crate::systems::plumbing::WaterStatus;

/// Ticks in one run. Enough at GARDEN_SPEED for several game days (light, dark and the stages).
const TICKS: usize = 200;
/// The clock's speed in the 20-minute day's units (set_garden_speed): growth fast enough that
/// crops cross stages, pests build and weeds come within TICKS ticks of one real second each.
const GARDEN_SPEED: f32 = 400.0;
/// The recorded digest, relative to the crate root.
const GOLDEN: &str = "src/systems/farming/golden/farming_state.txt";

/// The areas every plant is sown in: a bed in each of two grow rooms, a tower slot, and an outdoor
/// field (a name ending `_field_<n>` is a field, `is_field_area`).
const AREAS: [&str; 4] = ["bed_a", "bed_b", "aeroponic_tower_1", "veg_field_1"];

/// The channels the tick reads, two grow rooms of different size, the four areas as plots, pests
/// and weeds at a middling severity.
fn store() -> DataStore {
    let mut data = make_store();
    set_garden_speed(&data, GARDEN_SPEED);
    data.insert("garden_pest_severity", Mutex::new(0.5_f32));
    data.insert("player_notices", Mutex::new(Vec::<String>::new()));
    data.insert("hand_water_draw_l", Mutex::new(0.0_f32));
    data.insert("irrigation_demand_lpm", Mutex::new(0.0_f32));
    data.insert("water_status", Mutex::new(WaterStatus::default()));
    data.insert("pest_control_request", Mutex::new(Option::<(String, String)>::None));
    data.insert(super::weeds::REQUEST_KEY, Mutex::new(Option::<(String, String)>::None));
    data.insert(humidity::DATA_KEY, HumidityData::parse(HUMIDITY_RON).expect("humidity.ron"));
    data.insert(life_support::DATA_KEY, LifeSupportData::parse(LIFE_SUPPORT_RON).expect("life_support.ron"));
    let rooms = vec![
        GrowRoom { id: "room-a".into(), name: "Greenhouse".into(), min: [0.0, 0.0, 0.0], max: [10.0, 3.0, 10.0], ..Default::default() },
        GrowRoom { id: "room-b".into(), name: "Grow room".into(), min: [20.0, 0.0, 0.0], max: [24.0, 2.5, 6.0], ..Default::default() },
    ];
    data.insert(humidity::ROOMS_KEY, Mutex::new(rooms));
    let plots = vec![
        GrowPlot { id: "bed_a".into(), pos: [5.0, 0.0, 5.0], footprint_m2: 40.0, ..Default::default() },
        GrowPlot { id: "bed_b".into(), pos: [22.0, 0.0, 3.0], footprint_m2: 12.0, ..Default::default() },
        GrowPlot { id: "aeroponic_tower_1".into(), pos: [2.0, 0.0, 2.0], footprint_m2: 1.0, ..Default::default() },
        GrowPlot { id: "veg_field_1".into(), pos: [60.0, 0.0, 60.0], footprint_m2: 14.4, ..Default::default() },
    ];
    data.insert("grow_plots", plots);
    let areas: HashMap<String, f32> =
        [("bed_a", 10.0_f32), ("bed_b", 3.0), ("aeroponic_tower_1", 0.25), ("veg_field_1", 1.44)].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    data.insert(super::units::PLOT_AREA_KEY, areas);
    let instances: HashMap<String, Vec<(String, u32)>> =
        [("veg_field".to_string(), vec![("veg_field_1".to_string(), 10)])].into_iter().collect();
    data.insert("grow_instances", instances);
    data
}

/// Every plant in the registry, in id order, sown in every area on its own slot, the sowing times
/// spread so the garden holds every stage at once. Plus the home's air, two air handlers, the
/// irrigation and a cistern, as the life-support tests build them.
fn world(data: &DataStore) -> (hecs::World, usize) {
    let reg = data.get::<PlantRegistry>("plant_registry").unwrap();
    let mut ids: Vec<String> = reg.plants.keys().cloned().collect();
    ids.sort();
    let mut world = hecs::World::new();
    world.spawn((HomeAir { metabolic_kcal_per_day: 0.0, ..Default::default() }, EnclosedSpace::new_sealed(2000.0)));
    let mut sown = 0;
    for (i, plant) in ids.iter().enumerate() {
        let def = reg.get(plant).unwrap();
        for area in AREAS {
            world.spawn((CropInstance {
                crop_def_id: plant.clone(),
                growth_stage: def.first_stage().to_string(),
                planted_at: -(i as f64 % 9.0) * 3_600.0,
                water_level: 0.4 + (i % 7) as f32 * 0.1,
                health: 100.0,
                tower_id: Some(area.to_string()),
                tower_slot: Some(i as u32),
                health_seconds: 0.0,
                growing_seconds: 0.0,
            },));
            sown += 1;
        }
    }
    for at in [[5.0, 2.7, 1.0], [22.0, 2.2, 3.0]] {
        let e = world.spawn((Transform { position: glam::Vec3::from_array(at), ..Default::default() },));
        world
            .insert(
                e,
                (
                    AirHandler { airflow_m3_h: 1842.0, watts: 325.0 },
                    PowerConsumer { draw_watts: 0.0, priority: 2, enabled: true },
                    WaterProducer { lpm: 0.0, needs_power: true },
                    PlumbingCircuit { island: 0 },
                ),
            )
            .unwrap();
    }
    world.spawn((Irrigator, WaterConsumer { lpm: 0.0, needs_power: false }, PlumbingCircuit { island: 0 }));
    world.spawn((WaterTank { liters: 7000.0, capacity_l: 20000.0 }, PlumbingCircuit { island: 0 }));
    (world, sown)
}

/// The game clock moved on by one tick's game seconds, at the store's speed.
fn advance(data: &DataStore, dt: f32) {
    let gt = data.get::<Mutex<crate::systems::time::GameTime>>("game_time").unwrap();
    let mut g = gt.lock().unwrap();
    let t = g.elapsed_seconds + f64::from(dt) * f64::from(g.time_scale);
    g.set_elapsed(t);
}

/// Every object's keys sorted, all the way down, so the bytes never depend on map order.
fn canonical(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(m) => {
            let sorted: BTreeMap<String, serde_json::Value> = m.into_iter().map(|(k, v)| (k, canonical(v))).collect();
            serde_json::Value::Object(sorted.into_iter().collect())
        }
        serde_json::Value::Array(a) => serde_json::Value::Array(a.into_iter().map(canonical).collect()),
        other => other,
    }
}

/// What one run of the garden ends in, and how much happened in it.
struct Ended {
    state: String,
    sown: usize,
    past_first_stage: usize,
    pest_areas: usize,
    rooms_tracked: usize,
}

fn run() -> Ended {
    let data = store();
    let (mut world, sown) = world(&data);
    let mut sys = FarmingSystem::new();
    let dt = 1.0_f32;
    for _ in 0..TICKS {
        advance(&data, dt);
        sys.tick(&mut world, dt, &data);
    }
    let reg = data.get::<PlantRegistry>("plant_registry").unwrap();
    let mut crops: Vec<(u64, serde_json::Value)> = world
        .query::<(&CropInstance, Option<&CropSoil>)>()
        .iter()
        .map(|(e, (c, s))| (e.to_bits().get(), serde_json::json!({ "crop": c, "soil": s })))
        .collect();
    crops.sort_by_key(|(id, _)| *id);
    let past_first_stage = world
        .query::<&CropInstance>()
        .iter()
        .filter(|(_, c)| reg.get(&c.crop_def_id).is_some_and(|d| c.growth_stage != d.first_stage()))
        .count();
    let mut memories: Vec<(u64, SoilMemory)> = world.query::<&SoilMemory>().iter().map(|(e, m)| (e.to_bits().get(), m.clone())).collect();
    memories.sort_by_key(|(id, _)| *id);
    let pest_areas = memories.iter().map(|(_, m)| m.pests.values().filter(|p| serde_json::to_value(p).map_or(false, |v| v != serde_json::json!({}))).count()).sum();
    let rooms_tracked = memories.iter().map(|(_, m)| m.rooms.len()).sum();
    let all = serde_json::json!({
        "crops": crops.into_iter().map(|(_, v)| v).collect::<Vec<_>>(),
        "memory": memories.into_iter().map(|(_, m)| serde_json::to_value(m).expect("SoilMemory serializes")).collect::<Vec<_>>(),
    });
    Ended { state: serde_json::to_string(&canonical(all)).expect("the state serializes"), sown, past_first_stage, pest_areas, rooms_tracked }
}

/// THE GARDEN ENDS IN THE SAME STATE EVERY RUN, AND IN THE RECORDED ONE (the proof the speed work
/// F1 and F2 changes no result; docs/design/sim-frame-cost.md). See the module comment.
#[test]
fn the_garden_ends_in_the_same_state_every_run_and_in_the_recorded_one() {
    let a = run();
    let b = run();
    assert_eq!(a.state.len(), b.state.len(), "the same garden built twice ended in states of different size");
    if a.state != b.state {
        let at = a.state.bytes().zip(b.state.bytes()).position(|(x, y)| x != y).unwrap_or(0);
        panic!(
            "the same garden built twice ended differently (the tick is not deterministic), first at byte {at}: ...{}... vs ...{}...",
            &a.state[at.saturating_sub(80)..(at + 80).min(a.state.len())],
            &b.state[at.saturating_sub(80)..(at + 80).min(b.state.len())]
        );
    }
    // The garden did something worth recording.
    // 189 plants in data/plants.csv on 2026-10-30, so 756 crops; a registry that shrank badly would
    // make the golden cover little.
    assert!(a.sown >= 600 && a.sown % AREAS.len() == 0, "a garden of every plant in four areas: {} crops", a.sown);
    assert!(a.past_first_stage * 5 > a.sown, "crops grew: {} of {} past their first stage", a.past_first_stage, a.sown);
    assert!(a.pest_areas > 0, "pests arrived somewhere");
    assert!(a.rooms_tracked >= 2, "both grow rooms' air was tracked: {}", a.rooms_tracked);

    let digest = blake3::hash(a.state.as_bytes()).to_hex().to_string();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(GOLDEN);
    let line = format!("blake3 {digest} crops {} ticks {TICKS} speed {GARDEN_SPEED}", a.sown);
    if std::env::var("HUMANITY_WRITE_GOLDEN").is_ok_and(|v| v == "1") {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, format!("{line}\n")).unwrap();
        println!("wrote the golden: {line}");
        return;
    }
    let recorded = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("no golden at {GOLDEN}: run once with HUMANITY_WRITE_GOLDEN=1 on the code you trust, and commit it"));
    assert_eq!(
        recorded.trim(),
        line,
        "the garden ended in a different state from the recorded golden. A speed change must not do this; a change meant to alter the garden reruns with HUMANITY_WRITE_GOLDEN=1 and says why"
    );
}

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

use std::collections::BTreeMap;
use std::sync::Mutex;

use super::*;
use super::fixtures::golden::*;
use humanity_core::ecs::components::{
    CropInstance, CropSoil, SoilMemory,
};
use humanity_core::ecs::systems::System;
use humanity_core::hot_reload::data_store::DataStore;

/// Ticks in one run. Enough at GARDEN_SPEED for several game days (light, dark and the stages).
const TICKS: usize = 200;
/// The recorded digest, relative to the repo root (HUMANITY_REPO_ROOT).
const GOLDEN: &str = "crates/humanity-systems/src/farming/golden/farming_state.txt";

/// The game clock moved on by one tick's game seconds, at the store's speed.
fn advance(data: &DataStore, dt: f32) {
    let gt = data.get::<Mutex<crate::time::GameTime>>("game_time").unwrap();
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
    /// The garden's change numbers at the end (versions.rs): which kind of run the body made.
    versions: super::versions::Versions,
}

/// The golden garden run TICKS ticks, with the farm's interval (rung 2, farm-clock.md) published
/// as given, or none (every frame, the farm as it ran before rung 2).
fn run_at(interval_s: Option<f64>) -> Ended {
    let mut data = store();
    if let Some(s) = interval_s {
        data.insert(super::clock::INTERVAL_KEY, Mutex::new(s));
    }
    // The change numbers (versions.rs): they record the runs and change nothing the farm does.
    super::versions::register(&mut data);
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
    let versions = super::versions::read(&data, |v| v.clone()).expect("registered above");
    Ended { state: serde_json::to_string(&canonical(all)).expect("the state serializes"), sown, past_first_stage, pest_areas, rooms_tracked, versions }
}

/// THE GARDEN ENDS IN THE SAME STATE EVERY RUN, AND IN THE RECORDED ONE (the proof the speed work
/// F1 and F2 changes no result; docs/design/sim-frame-cost.md). See the module comment.
#[test]
fn the_garden_ends_in_the_same_state_every_run_and_in_the_recorded_one() {
    let a = run_at(None);
    let b = run_at(None);
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
    let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join(GOLDEN);
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

/// A FRAME LONGER THAN THE INTERVAL RUNS THE WHOLE FARM AS ONE (2026-10-31, docs/design/
/// farm-clock.md rung 2; restated the same day after the seam review's C). The golden garden's
/// ticks are 8 game hours each (speed 400: 28,800 game seconds in each 1 s frame), far past every
/// interval Settings offers, so `clock::plan` brings up every area every tick and the farm's body
/// runs once over the whole farm, exactly as with no interval: that, and only that, is what this
/// shows, so the golden digest holds with the default interval published. It was named "the golden
/// state holds at the default interval" as though it exercised the turns there; it never could. The
/// turns themselves (frames shorter than the interval, each area on its own turn, against the farm
/// advancing every area every frame) are clock_tests.rs's: `each_area_advances_once_an_interval`,
/// `every_second_and_every_minute_end_where_every_frame_ends`,
/// `ten_minutes_and_an_hour_end_where_every_frame_ends` and the readers' tests.
///
/// That the turn path never ran is checked, not assumed: every run of the body moves the garden's
/// change numbers (versions.rs), a whole-farm run the epoch and a run over named areas each area's
/// own, and here no area's own number ever moves. Seen red 2026-10-31 with the interval published as
/// `clock::MAX_INTERVAL_S` (a day, longer than a tick: the four areas then advance a day at a time
/// every third tick, still all together as one) by the end it reaches, "left: 272037 right: 273520"
/// (the state's length against no interval's).
#[test]
fn a_frame_longer_than_the_interval_runs_the_whole_farm_as_one() {
    let every_frame = run_at(None);
    let default = run_at(Some(super::clock::DEFAULT_INTERVAL_S));
    assert!(default.versions.epoch > 0, "the body ran");
    assert!(!default.versions.any_area(), "a run over named areas: the turns ran, which this garden's ticks cannot make them do");
    assert_eq!(default.state.len(), every_frame.state.len());
    assert!(default.state == every_frame.state, "the whole farm in one run ended differently from no interval at all");
    let digest = blake3::hash(default.state.as_bytes()).to_hex().to_string();
    let recorded = std::fs::read_to_string(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join(GOLDEN)).expect("the golden");
    assert!(recorded.contains(&digest), "the default interval's digest {digest} is not the recorded one: {recorded}");
}

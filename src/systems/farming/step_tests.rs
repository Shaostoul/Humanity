//! THE GARDEN'S RESULTS DO NOT DEPEND ON THE LENGTH OF ITS TICK (docs/design/farm-clock.md, rung
//! 1, 2026-10-30). A 30 fps and a 120 fps player must grow the same garden, and rung 2 (the farm
//! on its own clock, advanced every second or every minute) and rung 4 (a server's home caught up
//! in one step when someone looks at it) are only right if a long step lands where many short
//! ones do.
//!
//! The garden is the golden one (golden_tests.rs: every plant in plants.csv in four areas, two
//! grow rooms, pests and weeds at the gentle severity), on the real-time clock, made to cross every
//! regime a step can span:
//! - the clock starts at 05:20:34.5, so the long steps straddle sunrise, sunset and the grow
//!   lights' switch-off instead of landing on whole hours;
//! - a grow light over the tower, so the night has a lit part (the lamps' timer, 18:00 to 24:00);
//! - bed_b's irrigation is set to 0.15, below the 0.2 stress line, so each of its crops dries out,
//!   crosses into stress at its own moment and dies two game hours later;
//! - bed_a's units start empty with the feeder at a quarter (half the reserve), from an ample home
//!   stock, so the feeder fills them and then feeds every step's draw (urine for the nitrogen up
//!   to a month before harvest, compost after), and the crops settle toward the ceiling that half
//!   sufficiency allows;
//! - every crop's age is spread over its season, so stages turn and crops ripen inside the span.
//!
//! The same 30 game hours run at a 60 s tick, a 600 s tick and a 3,600 s tick (the dt ladder of
//! docs/design/headless-sim-runner.md, section 3, from its 60 s rung up), and the ends are compared
//! crop by crop: the growth stage exactly, every number within the bound `tolerance` states.
//!
//! Twice: without the air the farm tracks (no grow rooms, no home air space), where rung 1 claims
//! every step exact and the bounds are rounding; and with it, the golden garden whole, where one
//! coupling is still taken a step at a time. The crops' humidity and CO2 caps read the rooms' air
//! as it stood at the step's start, and the air steps after the crops on the step's average breath
//! (humidity::step_rooms, its own 15-minute slices): first order in the step, and a room's air
//! moves in minutes. Rung 3 of farm-clock.md solves the air between events; until then the bounds
//! for it are the measured spread, stated.
//!
//! Each run first takes the same one-second tick: a fresh garden's first tick reads the home air
//! space's default humidity until the farm's first air step writes the modelled one, so a long
//! first step would carry that default for an hour.
//!
//! BEFORE rung 1: these two tests run on the farm code of 2026-10-30 as it was (b1ec466de), each
//! against its own 60 s run, at 600 s and at 3,600 s. Without the air: growing_seconds off by
//! 540 s and 3,540 s and health_seconds by 810 s and 5,310 s (8% and 50%: a crop dying partway
//! through a step was counted alive, at its end health, to the step's end), planted_at by 1,080 s
//! and 4,680 s (the light read at each step's last moment), a fed unit's nitrogen by 0.8% and 5%
//! and its potash by 0.07% and 1.1% (topped up once a step, then drawn for the whole step), the
//! uptake by 8e-4 and 5e-3, a nitrifying pH pool by 0.03% and 0.3%, and the field's cabbage
//! caterpillars there at one tick length and missing at the other (a short tick's arrivals were
//! under the forgetting line). With the air: 34 crops in a different stage at both (the tower's,
//! under the grow light), health_seconds off by 4% and 53%, the home's CO2 by 1.5% and 4.4%, the
//! tower's powdery mildew there at one length and missing at the other. Water alone matched: the
//! irrigation held every level it reached. Against the bounds in `tolerance` the old code fails
//! both tests at both tick lengths.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use super::golden_tests::{store as golden_store, world as golden_world};
use super::*;
use crate::ecs::components::{CropInstance, CropSoil, GrowLight, PowerConsumer, Transform};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;

/// Where the clock starts: 05:20:34.5 on day 0, so the first hour-long step spans the sunrise.
const START_S: f64 = 5.0 * 3600.0 + 1234.5;
/// The tick every run takes first, game seconds (see the module comment).
const WARM_S: f64 = 1.0;
/// The reference tick, game seconds.
const SHORT_S: f64 = 60.0;
/// How long each run is: a day and a quarter, two sunrises, a sunset and the lamps' switch-off.
const SPAN_S: f64 = 30.0 * 3600.0;
/// The longer ticks, as multiples of the reference one.
const MULTIPLES: [u32; 2] = [10, 60];

/// The golden garden on the real-time clock, made to cross every regime (see the module comment).
/// `air`: with the golden garden's grow rooms and the home's air space, or with no air tracked.
fn garden(air: bool) -> (DataStore, hecs::World) {
    let mut data = golden_store();
    {
        let gt = data.get::<Mutex<crate::systems::time::GameTime>>("game_time").unwrap();
        let mut g = gt.lock().unwrap();
        g.time_scale = 1.0;
        g.set_elapsed(START_S);
    }
    data.insert("garden_irrigation", Mutex::new(HashMap::from([("bed_b".to_string(), 0.15_f32)])));
    data.insert("garden_nutrient", Mutex::new(HashMap::from([("bed_a".to_string(), 0.25_f32)])));
    let doses = soil::NutrientData::parse(soil::NUTRIENTS_RON).expect("nutrients.ron");
    let stock: HashMap<String, u32> = doses.fertilizers.iter().map(|f| (f.item.clone(), 1_000_000)).collect();
    data.insert("home_stock", Mutex::new(stock));
    if !air {
        data.insert(humidity::ROOMS_KEY, Mutex::new(Vec::<humidity::GrowRoom>::new()));
    }
    let (mut world, _) = golden_world(&data);
    if !air {
        let homes: Vec<hecs::Entity> = world.query::<&crate::systems::atmosphere::HomeAir>().iter().map(|(e, _)| e).collect();
        for e in homes {
            world.despawn(e).unwrap();
        }
    }
    let reg = data.get::<PlantRegistry>("plant_registry").unwrap();
    let mut bed_a: Vec<hecs::Entity> = Vec::new();
    for (e, c) in world.query_mut::<&mut CropInstance>() {
        let growth_s = reg.get(&c.crop_def_id).map_or(0.0, |d| f64::from(d.growth_days) * SECONDS_PER_DAY);
        let slot = f64::from(c.tower_slot.unwrap_or(0));
        // Ten ages a plant apart, from just sown to 90% of the way through its season.
        c.planted_at = START_S - (slot % 10.0) / 10.0 * growth_s;
        if c.tower_id.as_deref() == Some("bed_a") {
            bed_a.push(e);
        }
    }
    for e in bed_a {
        world.insert_one(e, CropSoil { store: Npk::ZERO, uptake: 0.0 }).unwrap();
    }
    world.spawn((
        GrowLight { watts: 100.0 },
        PowerConsumer { draw_watts: 100.0, priority: 5, enabled: true },
        Transform { position: glam::Vec3::new(2.0, 2.0, 2.0), ..Default::default() },
    ));
    (data, world)
}

/// One crop's end state, the parts compared.
#[derive(Debug, Clone)]
struct Crop {
    plant: String,
    area: String,
    stage: String,
    water: f64,
    health: f64,
    health_s: f64,
    growing_s: f64,
    planted_at: f64,
    store: [f64; 3],
    uptake: f64,
}

/// Where one run ended: every crop in entity order, and the soil memory (pests, rooms, pH, weeds,
/// organic N, the home's air) as canonical JSON.
type Ended = (Vec<Crop>, serde_json::Value);

/// The garden run SPAN_S game seconds at ticks of `step_s`, after the warm-up tick.
fn run(step_s: f64, air: bool) -> Ended {
    let (data, mut world) = garden(air);
    let mut sys = FarmingSystem::new();
    let set_clock = |t: f64| data.get::<Mutex<crate::systems::time::GameTime>>("game_time").unwrap().lock().unwrap().set_elapsed(t);
    set_clock(START_S + WARM_S);
    sys.tick(&mut world, WARM_S as f32, &data);
    let n = (SPAN_S / step_s).round() as usize;
    for k in 1..=n {
        set_clock(START_S + WARM_S + k as f64 * step_s);
        sys.tick(&mut world, step_s as f32, &data);
    }
    let mut crops: Vec<(u64, Crop)> = world
        .query::<(&CropInstance, Option<&CropSoil>)>()
        .iter()
        .map(|(e, (c, s))| {
            let store = s.map_or([0.0; 3], |s| [s.store.n, s.store.p2o5, s.store.k2o]);
            (
                e.to_bits().get(),
                Crop {
                    plant: c.crop_def_id.clone(),
                    area: c.tower_id.clone().unwrap_or_default(),
                    stage: c.growth_stage.clone(),
                    water: f64::from(c.water_level),
                    health: f64::from(c.health),
                    health_s: c.health_seconds,
                    growing_s: c.growing_seconds,
                    planted_at: c.planted_at,
                    store,
                    uptake: f64::from(s.map_or(0.0, |s| s.uptake)),
                },
            )
        })
        .collect();
    crops.sort_by_key(|(id, _)| *id);
    let memory: Vec<serde_json::Value> =
        world.query::<&crate::ecs::components::SoilMemory>().iter().map(|(_, m)| serde_json::to_value(m).unwrap()).collect();
    (crops.into_iter().map(|(_, c)| c).collect(), canonical(serde_json::Value::Array(memory)))
}

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

/// The largest difference seen in one quantity, and where.
#[derive(Debug, Default, Clone)]
struct Worst {
    abs: f64,
    rel: f64,
    at: String,
}

impl Worst {
    fn see(&mut self, a: f64, b: f64, at: impl FnOnce() -> String) {
        let abs = (a - b).abs();
        let rel = abs / a.abs().max(b.abs()).max(1e-300);
        if abs > self.abs {
            self.abs = abs;
            self.rel = rel;
            self.at = at();
        }
    }
}

/// Every numeric leaf of two JSON trees compared, grouped by the memory field it sits under and its
/// own key ("memory.pests.level"); a key one side lacks is compared against zero on the other.
fn json_diff(a: &serde_json::Value, b: &serde_json::Value, path: &str, group: &str, out: &mut BTreeMap<String, Worst>) {
    use serde_json::Value as V;
    let zero = V::from(0.0);
    match (a, b) {
        (V::Object(x), V::Object(y)) => {
            let keys: std::collections::BTreeSet<&String> = x.keys().chain(y.keys()).collect();
            for k in keys {
                let g = if group.is_empty() { k.as_str() } else { group };
                json_diff(x.get(k).unwrap_or(&zero), y.get(k).unwrap_or(&zero), &format!("{path}.{k}"), g, out);
            }
        }
        (V::Array(x), V::Array(y)) => {
            for i in 0..x.len().max(y.len()) {
                json_diff(x.get(i).unwrap_or(&zero), y.get(i).unwrap_or(&zero), &format!("{path}[{i}]"), group, out);
            }
        }
        (V::Object(x), z) | (z, V::Object(x)) if z.as_f64() == Some(0.0) => {
            for (k, v) in x {
                let g = if group.is_empty() { k.as_str() } else { group };
                json_diff(v, &zero, &format!("{path}.{k}"), g, out);
            }
        }
        _ => {
            if let (Some(p), Some(q)) = (a.as_f64(), b.as_f64()) {
                let leaf = path.rsplit('.').next().unwrap_or(path);
                let leaf = leaf.split('[').next().unwrap_or(leaf);
                out.entry(format!("memory.{group}.{leaf}")).or_default().see(p, q, || path.to_string());
            }
        }
    }
}

/// How two runs' ends differ: stage mismatches by name, and the worst difference per quantity.
fn compare(base: &Ended, other: &Ended) -> (Vec<String>, BTreeMap<String, Worst>) {
    let mut stages = Vec::new();
    let mut worst: BTreeMap<String, Worst> = BTreeMap::new();
    assert_eq!(base.0.len(), other.0.len(), "the same crops in both runs");
    for (i, (a, b)) in base.0.iter().zip(other.0.iter()).enumerate() {
        assert_eq!((&a.plant, &a.area), (&b.plant, &b.area), "crop {i} is the same plant in both runs");
        let at = || format!("{} in {} (crop {i})", a.plant, a.area);
        if a.stage != b.stage {
            stages.push(format!("{}: {} vs {}", at(), a.stage, b.stage));
        }
        worst.entry("crop.water".into()).or_default().see(a.water, b.water, at);
        worst.entry("crop.health".into()).or_default().see(a.health, b.health, at);
        worst.entry("crop.health_seconds".into()).or_default().see(a.health_s, b.health_s, at);
        worst.entry("crop.growing_seconds".into()).or_default().see(a.growing_s, b.growing_s, at);
        worst.entry("crop.planted_at".into()).or_default().see(a.planted_at, b.planted_at, at);
        for (k, name) in ["n", "p2o5", "k2o"].iter().enumerate() {
            worst.entry(format!("soil.store.{name}")).or_default().see(a.store[k], b.store[k], at);
        }
        worst.entry("soil.uptake".into()).or_default().see(a.uptake, b.uptake, at);
    }
    json_diff(&base.1, &other.1, "memory", "", &mut worst);
    (stages, worst)
}

/// What a run did, so a pass is not a pass of an inert garden: crops that died, that changed
/// stage, and living crops below 99 health.
fn activity(r: &Ended, start: &[String]) -> (usize, usize, usize) {
    let dead = r.0.iter().filter(|c| c.stage == crate::ecs::components::STAGE_DEAD).count();
    let moved = r.0.iter().zip(start).filter(|(c, s)| c.stage != **s && c.stage != crate::ecs::components::STAGE_DEAD).count();
    let below = r.0.iter().filter(|c| c.health < 99.0 && c.stage != crate::ecs::components::STAGE_DEAD).count();
    (dead, moved, below)
}

/// Run the garden at every tick length and hold each quantity to `tolerance(quantity, air)`.
fn the_garden_ends_the_same(air: bool) {
    let (_, w0) = garden(air);
    let start: Vec<String> = {
        let mut v: Vec<(u64, String)> = w0.query::<&CropInstance>().iter().map(|(e, c)| (e.to_bits().get(), c.growth_stage.clone())).collect();
        v.sort_by_key(|(id, _)| *id);
        v.into_iter().map(|(_, s)| s).collect()
    };
    let base = run(SHORT_S, air);
    let (dead, moved, below) = activity(&base, &start);
    let which = if air { "with the air" } else { "without the air" };
    println!("{which}, reference ({SHORT_S} s ticks): {} crops, {dead} died, {moved} changed stage, {below} living below 99 health", base.0.len());
    let mut failures = Vec::new();
    for m in MULTIPLES {
        let step = SHORT_S * f64::from(m);
        let other = run(step, air);
        let (stages, worst) = compare(&base, &other);
        println!("--- {which}, {step} s ticks ({m}x) against {SHORT_S} s: {} stage mismatches", stages.len());
        for s in stages.iter().take(8) {
            println!("    stage: {s}");
        }
        for (k, w) in &worst {
            if w.abs > 0.0 {
                println!("    {k}: max abs {:.3e} (rel {:.3e}) at {}", w.abs, w.rel, w.at);
            }
        }
        if !stages.is_empty() {
            failures.push(format!("{m}x: {} crops ended in a different stage, first {}", stages.len(), stages[0]));
        }
        for (k, w) in &worst {
            let (abs_tol, rel_tol) = tolerance(k, air, step);
            if w.abs > abs_tol && w.rel > rel_tol {
                failures.push(format!("{m}x: {k} differs by {:.3e} (rel {:.3e}) at {}, over {abs_tol:e} / {rel_tol:e}", w.abs, w.rel, w.at));
            }
        }
    }
    assert!(dead > 0 && moved > 0 && below > 0, "the garden crossed its regimes: {dead} died, {moved} changed stage, {below} below 99 health");
    assert!(failures.is_empty(), "the garden's end depends on the tick length ({which}):\n{}", failures.join("\n"));
}

/// WITHOUT THE AIR, EVERY STEP IS EXACT: the same garden, the same 30 game hours, at a 60 s, a
/// 600 s and a 3,600 s tick, ends in the same stage everywhere and the same numbers to rounding.
#[test]
fn the_garden_ends_the_same_at_any_tick_length() {
    the_garden_ends_the_same(false);
}

/// WITH THE AIR, THE ONE COUPLING STILL TAKEN A STEP AT A TIME STAYS WITHIN ITS MEASURED BOUND:
/// the golden garden whole (its grow rooms and the home's air space), as above.
#[test]
fn the_garden_with_its_air_ends_within_the_air_couplings_bound() {
    the_garden_ends_the_same(true);
}

/// Each quantity's bound against the 60 s run, absolute and relative: a difference passes if it
/// is within EITHER. Measured spreads at the 3,600 s tick (2026-10-30) in brackets.
///
/// Without the air every crop number is rounding: water, health and the uptake are f32s stored
/// every tick (a 60 s run rounds 1,800 times, a 3,600 s run 30), so a stress crossing moves by
/// hundredths of a second [growing_seconds 0.039 s] and a health by an ulp [7.6e-6]. What is left
/// above rounding is first order in the step and small, each listed in farm-clock.md rung 1: the
/// pests and weeds read the crops' state at the step's start, which moves a field crop's caps by
/// thousandths [health_seconds rel 2.5e-5, aphids rel 2.5e-3 at a millionth of a full
/// infestation, weed cover rel 3.3e-5]; the feeder's dose for a step's draw lands at the step's
/// start, so compost's slow N and urine's acidity start working a step early [organic N rel
/// 2.5e-4, a unit's N rel 2.1e-3 where compost overshoots it, a nitrifying pool rel 4.8e-4], and
/// a new bank's mean cohort age is averaged in before it ages [age_days rel 4.2e-2: it only
/// decides when a 30-day cohort closes].
///
/// With the air the crops' humidity and CO2 caps lag the room's air by a step (see the module
/// comment): a crop easing toward a cap that is moving sees it a step late, so a dying crop's
/// death moves by about a tenth of the step (the nutrient decline rate over the drought decay
/// rate) [354 s at 3,600 s, 54 s at 600 s], and the air itself, stepped in its own slices on the
/// step's average breath, is first order too [the home's CO2 rel 3.7e-2 at 3,600 s, 3.1e-3 at
/// 600 s]. Those bounds grow with the step.
fn tolerance(key: &str, air: bool, step_s: f64) -> (f64, f64) {
    let exact = match key {
        "crop.water" | "soil.uptake" => (1e-6, 0.0),
        "crop.health" => (1e-4, 0.0),
        "crop.health_seconds" => (0.0, 1e-4),
        "crop.growing_seconds" | "crop.planted_at" => (0.5, 0.0),
        "soil.store.n" => (0.0, 5e-3),
        "soil.store.p2o5" | "soil.store.k2o" => (0.0, 1e-4),
        "memory.organic.age_days" => (0.0, 0.1),
        "memory.organic.n" => (0.0, 1e-3),
        "memory.pests.level" => (1e-6, 1e-2),
        "memory.ph.nitrification" => (0.0, 2e-3),
        "memory.ph.ph" => (0.0, 1e-5),
        k if k.starts_with("memory.weeds.") => (0.0, 1e-4),
        _ => (0.0, 1e-9),
    };
    if !air {
        return exact;
    }
    // The air's coupling, first order in the step: per second of it.
    match key {
        "crop.growing_seconds" | "crop.planted_at" => (0.15 * step_s, 0.0),
        "crop.health_seconds" => (0.25 * step_s, 0.0),
        "soil.uptake" => (2e-7 * step_s, 0.0),
        "soil.store.n" | "soil.store.p2o5" | "soil.store.k2o" => (0.0, 5e-3_f64.max(2e-7 * step_s)),
        "memory.pests.level" => (1e-4, 0.0),
        k if k.starts_with("memory.home_air.") || k.starts_with("memory.rooms.") => (0.0, 2e-5 * step_s),
        _ => exact,
    }
}

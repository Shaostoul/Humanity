//! The body at frame rate (BUG-164, 2026-10-05): hunger, thirst, tiredness, waste, starving,
//! an illness's water and its countdown, driven through the FoodSystem's own tick one frame
//! at a time, at time speed 1.
//!
//! The other body tests step minutes or hours per tick, where a frame's rounding is far below
//! the change. The game steps a sixtieth of a second or less: at time speed 1 a frame's share
//! of a day's thirst is a few millionths of a point, below what an f32 near 100 can hold, and
//! a frame's share of a two-day illness is below what an f32 at 172,800 s can hold. Every run
//! here ticks the real FoodSystem at 60, 144 or 240 frames a second, from a full stomach.
//!
//! TWO LENGTHS. A tick of this system costs about 40 microseconds in a debug build (measured
//! 2026-10-05: the DataStore lookups and world queries of an unoptimised build), so twelve
//! game hours at 240 frames a second is about seven minutes, and a whole two-day illness
//! about twenty. The runs that always run (`..._for_ten_minutes`) take ten game minutes at
//! the top of every range, where an f32's steps are coarsest and the defect was worst, and
//! check each change to within one percent of its rate. The full runs (`..._for_the_whole_
//! course`, ignored by default) take twelve game hours and then the illness to its end:
//! `cargo test --features native --lib -- --ignored frame_rate_tests`.
//!
//! A child of `food` (`use super::*`), so it reads the FoodSystem's own record of the sweat
//! its body heat model made.

use super::*;
use crate::ecs::components::{Controllable, Health, StatusEffects, Vitals};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::illness::{Mode, MODE_KEY};
use crate::systems::inventory::Inventory;
use crate::systems::status_effects::StatusEffectRegistry;
use std::sync::Mutex;

fn data_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/data"))
}

fn registry() -> StatusEffectRegistry {
    StatusEffectRegistry::from_csv(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/status_effects.csv")))
        .expect("status_effects.csv")
}

/// Game seconds in an hour.
const HOUR_S: f64 = 3600.0;
/// The short runs: ten game minutes.
const SHORT_S: f64 = 600.0;
/// The full runs read their numbers after twelve game hours.
const LONG_S: f64 = 12.0 * HOUR_S;
/// The waste meter's start: high in its range, where an f32's steps are
/// coarsest, and below the Unsanitary line (75).
const WASTE_START: f32 = 70.0;

/// The DataStore as lib.rs wires it for the FoodSystem, at time speed 1 (a game second is a
/// real one, the default), in Illness `mode`.
fn store(mode: Mode) -> DataStore {
    let mut data = DataStore::new();
    data.insert("status_effect_registry", registry());
    for slot in ["consume_request", "drink_request", "use_item_request"] {
        data.insert(slot, Mutex::new(Option::<String>::None));
    }
    data.insert("rest_request", Mutex::new(false));
    data.insert("compost_request", Mutex::new(false));
    data.insert("player_death", Mutex::new(Option::<String>::None));
    data.insert("player_notices", Mutex::new(Vec::<String>::new()));
    data.insert(MODE_KEY, mode);
    let mut clock = crate::systems::time::GameTime::default();
    clock.time_scale = 1.0;
    data.insert("game_time", Mutex::new(clock));
    data
}

/// An adult with a full stomach, full Hydration and full energy, the core at its comfortable
/// 36.8 C, and `waste` on the waste meter.
fn full_adult(world: &mut hecs::World, waste: f32) -> hecs::Entity {
    let vitals = Vitals {
        satiation: 100.0,
        hydration: 100.0,
        energy: 100.0,
        waste,
        body_temp_c: crate::systems::body_heat::CORE_NEUTRAL_C as f32,
        ..Vitals::default()
    };
    world.spawn((Controllable, Inventory::new(8), vitals, StatusEffects::default(), Health::default()))
}

fn request(data: &DataStore, channel: &str, item_id: &str) {
    *data.get::<Mutex<Option<String>>>(channel).unwrap().lock().unwrap() = Some(item_id.to_string());
}

/// `who` eats a roast chicken, spoiled when `spoiled` (spoiled food always poisons), through
/// the Eat button's channel and a tick of no time.
fn eat_chicken(sys: &mut FoodSystem, world: &mut hecs::World, data: &DataStore, who: hecs::Entity, spoiled: bool) {
    let keeps = sys.freshness_secs("roast_chicken_0").expect("roast chicken is food");
    {
        let mut inv = world.get::<&mut Inventory>(who).unwrap();
        inv.add_item("roast_chicken_0", 1, 99);
        let stack = inv.slots.iter_mut().flatten().find(|s| s.item_id == "roast_chicken_0").unwrap();
        stack.age_s = if spoiled { keeps + 1.0 } else { 0.0 };
    }
    request(data, "consume_request", "roast_chicken_0");
    sys.tick(world, 0.0, data);
}

fn ill(world: &hecs::World, who: hecs::Entity) -> bool {
    world.get::<&StatusEffects>(who).unwrap().has("food_poisoning")
}

/// Game seconds of the illness still to run.
fn illness_left(world: &hecs::World, who: hecs::Entity) -> f64 {
    world.get::<&StatusEffects>(who).unwrap().remaining("food_poisoning").unwrap_or(0.0)
}

fn vitals(world: &hecs::World, who: hecs::Entity) -> Vitals {
    (*world.get::<&Vitals>(who).unwrap()).clone()
}

fn health(world: &hecs::World, who: hecs::Entity) -> f64 {
    f64::from(world.get::<&Health>(who).unwrap().current)
}

/// Litres of sweat the FoodSystem's body heat model has taken from `who` so far.
fn sweat_l(sys: &FoodSystem, who: hecs::Entity) -> f64 {
    sys.body_heat.get(&who).map_or(0.0, |t| t.body.sweat_litres)
}

/// What a frame-rate run read after `read_at_s` game seconds.
struct Readings {
    /// The game seconds the run had actually stepped: the sum of its frames.
    at_s: f64,
    /// The well adult: satiation's fall, Hydration's fall, the sweat (in Hydration points)
    /// inside that fall, energy's fall and the waste meter's rise.
    satiation_fall: f64,
    hydration_fall: f64,
    sweat_points: f64,
    energy_fall: f64,
    waste_rise: f64,
    /// The ill adult: Hydration's fall, and the illness's time left taken off.
    ill_hydration_fall: f64,
    illness_taken_s: f64,
    /// The starving adult's health lost (short runs only).
    starving_health_lost: Option<f64>,
    /// Game seconds from falling ill to the illness passing, if it passed in the run.
    passed_after_s: Option<f64>,
}

/// A world of adults with full stomachs at the same comfortable start: one made ill by a
/// spoiled roast chicken, one fed the same meal fresh, and (when `starving`) one with nothing
/// in its stomach at all. Ticked at `hz` frames a game second at time speed 1, read after
/// `read_at_s` game seconds, and run on until the illness passes or `until_s`.
fn frame_run(mode: Mode, hz: u32, read_at_s: f64, until_s: f64, starving: bool) -> Readings {
    let mut sys = FoodSystem::new(data_dir());
    let data = store(mode);
    let mut world = hecs::World::new();
    let waste = WASTE_START;
    let sick = full_adult(&mut world, waste);
    let well = full_adult(&mut world, waste);
    eat_chicken(&mut sys, &mut world, &data, sick, true);
    eat_chicken(&mut sys, &mut world, &data, well, false);
    let hungry = starving.then(|| {
        let e = full_adult(&mut world, waste);
        world.get::<&mut Vitals>(e).unwrap().satiation = 0.0;
        e
    });
    assert!(ill(&world, sick) && !ill(&world, well), "the setup: one ill, one well");
    let (sick0, well0) = (vitals(&world, sick), vitals(&world, well));
    assert_eq!((sick0.hydration, well0.hydration, well0.satiation), (100.0, 100.0, 100.0), "both start full");
    let left0 = illness_left(&world, sick);

    let dt = 1.0 / hz as f32;
    let mut t = 0.0_f64;
    let mut read: Option<Readings> = None;
    let mut passed_after_s = None;
    while t < until_s.max(read_at_s) {
        sys.tick(&mut world, dt, &data);
        t += f64::from(dt);
        if passed_after_s.is_none() && !ill(&world, sick) {
            passed_after_s = Some(t);
        }
        if read.is_none() && t >= read_at_s {
            let (s, w) = (vitals(&world, sick), vitals(&world, well));
            read = Some(Readings {
                at_s: t,
                satiation_fall: f64::from(well0.satiation - w.satiation),
                hydration_fall: f64::from(well0.hydration - w.hydration),
                sweat_points: sweat_l(&sys, well) * f64::from(HYDRATION_PER_LITRE),
                energy_fall: f64::from(well0.energy - w.energy),
                waste_rise: f64::from(w.waste - well0.waste),
                ill_hydration_fall: f64::from(sick0.hydration - s.hydration),
                illness_taken_s: left0 - illness_left(&world, sick),
                starving_health_lost: hungry.map(|h| 100.0 - health(&world, h)),
                passed_after_s: None,
            });
            // The rest of the run is the illness's countdown alone.
            let _ = world.despawn(well);
            if let Some(h) = hungry {
                let _ = world.despawn(h);
            }
        }
        if read.is_some() && passed_after_s.is_some() {
            break;
        }
    }
    let mut r = read.expect("the run lasted until its readings");
    r.passed_after_s = passed_after_s;
    r
}

/// Within one percent of `want`.
fn within_1pc(got: f64, want: f64) -> bool {
    (got - want).abs() <= 0.01 * want.abs()
}

/// Run `mode` at `hz` and check every reading against its rate: from a full stomach,
/// satiation, Hydration (with the sweat the body heat model made), energy and the waste meter
/// move at the food system's rates, a starving adult loses health at starvation's rate, the
/// illness takes the water its data gives and its countdown runs with the clock. With
/// `whole_course`, the readings come after twelve game hours, and the illness must then pass
/// within a minute of its course.
fn check(mode: Mode, hz: u32, whole_course: bool) {
    let ill_data = crate::systems::illness::Illnesses::load(data_dir());
    let fp = ill_data.get("food_poisoning").expect("food_poisoning in illnesses.ron").clone();
    let course_s = f64::from(registry().duration("food_poisoning")) * f64::from(ill_data.course_share(mode));
    let (read_at_s, until_s) = if whole_course { (LONG_S, course_s + 120.0) } else { (SHORT_S, SHORT_S) };
    let r = frame_run(mode, hz, read_at_s, until_s, !whole_course);

    let mut wrong = Vec::new();
    let mut rate = |what: &str, got: f64, want: f64| {
        if !within_1pc(got, want) {
            wrong.push(format!("{what} {got:.4}, its rate says {want:.4}"));
        }
    };
    rate("satiation fell", r.satiation_fall, f64::from(SATIATION_DECAY_PER_SEC) * r.at_s);
    rate(
        "Hydration fell",
        r.hydration_fall,
        f64::from(HYDRATION_DECAY_PER_SEC) * r.at_s + r.sweat_points,
    );
    rate("energy fell", r.energy_fall, f64::from(ENERGY_DECAY_PER_SEC) * r.at_s);
    rate("the waste meter rose", r.waste_rise, f64::from(WASTE_RISE_PER_SEC) * r.at_s);
    if let Some(lost) = r.starving_health_lost {
        rate("starving took health", lost, f64::from(STARVE_DAMAGE_PER_SEC) * r.at_s);
    }
    let water_want = f64::from(fp.water_l_per_day) * f64::from(ill_data.water_share(mode))
        * f64::from(HYDRATION_PER_LITRE)
        * r.at_s
        / crate::systems::time::EARTH_DAY_S;
    rate("the illness took water (points)", r.ill_hydration_fall - r.hydration_fall, water_want);
    if (r.illness_taken_s - r.at_s).abs() > 1.0 {
        wrong.push(format!(
            "the illness's countdown ran {:.2} s while the clock ran {:.2} s",
            r.illness_taken_s, r.at_s
        ));
    }
    if whole_course {
        match r.passed_after_s {
            Some(s) if (s - course_s).abs() <= 60.0 => {}
            Some(s) => wrong.push(format!(
                "the illness passed after {:.3} h, its course is {:.3} h",
                s / HOUR_S,
                course_s / HOUR_S
            )),
            None => wrong.push(format!("the illness had not passed 2 minutes after its {:.0} h course", course_s / HOUR_S)),
        }
    }
    let span = if whole_course { "12 game hours and the whole course" } else { "10 game minutes" };
    assert!(wrong.is_empty(), "{mode:?} at {hz} frames a second, {span}: {}", wrong.join("; "));
}

#[test]
fn the_body_keeps_time_at_60_frames_a_second_for_ten_minutes_forgiving() {
    check(Mode::Forgiving, 60, false);
}

#[test]
fn the_body_keeps_time_at_60_frames_a_second_for_ten_minutes_realistic() {
    check(Mode::Realistic, 60, false);
}

#[test]
fn the_body_keeps_time_at_144_frames_a_second_for_ten_minutes_forgiving() {
    check(Mode::Forgiving, 144, false);
}

#[test]
fn the_body_keeps_time_at_144_frames_a_second_for_ten_minutes_realistic() {
    check(Mode::Realistic, 144, false);
}

#[test]
fn the_body_keeps_time_at_240_frames_a_second_for_ten_minutes_forgiving() {
    check(Mode::Forgiving, 240, false);
}

#[test]
fn the_body_keeps_time_at_240_frames_a_second_for_ten_minutes_realistic() {
    check(Mode::Realistic, 240, false);
}

#[test]
#[ignore = "12 game hours and a whole illness at frame rate: minutes in a debug build; run: cargo test --features native --lib -- --ignored frame_rate_tests"]
fn the_body_keeps_time_at_60_frames_a_second_for_the_whole_course_forgiving() {
    check(Mode::Forgiving, 60, true);
}

#[test]
#[ignore = "12 game hours and a whole illness at frame rate: minutes in a debug build; run: cargo test --features native --lib -- --ignored frame_rate_tests"]
fn the_body_keeps_time_at_60_frames_a_second_for_the_whole_course_realistic() {
    check(Mode::Realistic, 60, true);
}

#[test]
#[ignore = "12 game hours and a whole illness at frame rate: minutes in a debug build; run: cargo test --features native --lib -- --ignored frame_rate_tests"]
fn the_body_keeps_time_at_144_frames_a_second_for_the_whole_course_forgiving() {
    check(Mode::Forgiving, 144, true);
}

#[test]
#[ignore = "12 game hours and a whole illness at frame rate: minutes in a debug build; run: cargo test --features native --lib -- --ignored frame_rate_tests"]
fn the_body_keeps_time_at_144_frames_a_second_for_the_whole_course_realistic() {
    check(Mode::Realistic, 144, true);
}

#[test]
#[ignore = "12 game hours and a whole illness at frame rate: minutes in a debug build; run: cargo test --features native --lib -- --ignored frame_rate_tests"]
fn the_body_keeps_time_at_240_frames_a_second_for_the_whole_course_forgiving() {
    check(Mode::Forgiving, 240, true);
}

#[test]
#[ignore = "12 game hours and a whole illness at frame rate: minutes in a debug build; run: cargo test --features native --lib -- --ignored frame_rate_tests"]
fn the_body_keeps_time_at_240_frames_a_second_for_the_whole_course_realistic() {
    check(Mode::Realistic, 240, true);
}

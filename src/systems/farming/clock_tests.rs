//! THE FARM ON ITS OWN CLOCK (docs/design/farm-clock.md, rung 2, 2026-10-31): each grow area
//! advances once an interval, on its turn, by its own time since its last advance; what reads the
//! farm brings it up to now first; the Dev page's reuse switch turns all of it off.
//!
//! What is proved here:
//! - the rotation's rule (`plan`): a frame brings up the turns it crosses, every turn once an
//!   interval, everything for a frame longer than the interval, nothing for a held clock;
//! - THE SWITCH: with reuse off every area and the air advance every frame, and the garden ends
//!   to the bit where the farm with no interval ends;
//! - EQUIVALENCE: the step_tests garden (every regime crossed) run on the farm's own clock at an
//!   interval of a second and a minute (short, across a sunrise, frames of a tenth of a second)
//!   and of ten minutes, an hour and six hours (thirty game hours, frames of a minute) ends where
//!   the farm advancing every area every frame ends, within the bounds step_tests.rs established
//!   for a step of the interval's length (the air's coupling is first order in it);
//! - THE READERS: a harvest pressed between two turns of its area meets the crop as it is now
//!   (seen red with the tick not bringing the area up to now first); the save's ask brings every
//!   area and the air to now; a reader that looks keeps every area within a second and a turn.
//!
//! `farm_cost_per_frame` is ignored by default (it is a measurement, not a check, and it takes a
//! while in a test build): `just test-lib farm_cost_per_frame --ignored --nocapture`. A test build
//! is unoptimised, so its milliseconds are many times the release game's; before against after,
//! on the same machine and the same garden, is what it is for.

use std::sync::Mutex;

use super::golden_tests::store as golden_store;
use super::{gardening_tests, step_tests};
use super::*;
use crate::ecs::components::{AirHandler, CropInstance, Irrigator, PlumbingCircuit, PowerConsumer, Transform, WaterConsumer, WaterProducer, WaterTank};
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use crate::systems::atmosphere::{EnclosedSpace, HomeAir};

/// About the showcase's garden: 1,981 crops in 91 Garden groups (sim-frame-cost.md).
const CROPS: usize = 2_000;
const AREAS: usize = 90;
/// The background rig's frame rate (the game drops to 30 fps out of focus).
const FPS: f64 = 30.0;

/// The golden garden's channels on the real-time clock at 10:00, and 2,000 crops of every plant in
/// 90 areas: the golden garden's four (two grow rooms, a tower, a field) and 86 more towers, beds
/// and fields with no plot of their own, their ages spread over each plant's season, plus the
/// home's air, two air handlers, the irrigation and a cistern as the golden garden has them.
pub(super) fn showcase_size_garden() -> (DataStore, hecs::World) {
    let data = golden_store();
    {
        let gt = data.get::<Mutex<crate::systems::time::GameTime>>("game_time").unwrap();
        let mut g = gt.lock().unwrap();
        g.time_scale = 1.0;
        g.set_elapsed(10.0 * 3600.0);
    }
    let reg = data.get::<PlantRegistry>("plant_registry").unwrap();
    let mut ids: Vec<String> = reg.plants.keys().cloned().collect();
    ids.sort();
    let names: Vec<String> = (0..AREAS)
        .map(|k| match k {
            0 => "bed_a".to_string(),
            1 => "bed_b".to_string(),
            2 => "aeroponic_tower_1".to_string(),
            3 => "veg_field_1".to_string(),
            k if k % 3 == 0 => format!("aeroponic_tower_{}", k + 10),
            k if k % 3 == 1 => format!("bed_{k}"),
            k => format!("veg_field_{}", k + 10),
        })
        .collect();
    let mut world = hecs::World::new();
    world.spawn((HomeAir { metabolic_kcal_per_day: 2500.0, ..Default::default() }, EnclosedSpace::new_sealed(2000.0)));
    for i in 0..CROPS {
        let plant = &ids[i % ids.len()];
        let def = reg.get(plant).unwrap();
        let growth_s = f64::from(def.growth_days) * SECONDS_PER_DAY;
        let frac = (i % 10) as f64 / 10.0;
        world.spawn((CropInstance {
            crop_def_id: plant.clone(),
            growth_stage: stage_from_progress(frac as f32, &def.stages()).to_string(),
            planted_at: 10.0 * 3600.0 - frac * growth_s,
            water_level: 0.9,
            health: 100.0,
            tower_id: Some(names[i % AREAS].clone()),
            tower_slot: Some((i / AREAS) as u32),
            health_seconds: 0.0,
            growing_seconds: 0.0,
        },));
    }
    for at in [[5.0, 2.7, 1.0], [22.0, 2.2, 3.0]] {
        world.spawn((
            Transform { position: glam::Vec3::from_array(at), ..Default::default() },
            AirHandler { airflow_m3_h: 1842.0, watts: 325.0 },
            PowerConsumer { draw_watts: 0.0, priority: 2, enabled: true },
            WaterProducer { lpm: 0.0, needs_power: true },
            PlumbingCircuit { island: 0 },
        ));
    }
    world.spawn((Irrigator, WaterConsumer { lpm: 0.0, needs_power: false }, PlumbingCircuit { island: 0 }));
    world.spawn((WaterTank { liters: 7000.0, capacity_l: 20000.0 }, PlumbingCircuit { island: 0 }));
    (data, world)
}

/// The tick's own clock moved on by one frame, the way TimeSystem moves it before the farm ticks.
fn advance_clock(data: &DataStore, dt: f64) {
    let gt = data.get::<Mutex<crate::systems::time::GameTime>>("game_time").unwrap();
    let mut g = gt.lock().unwrap();
    let t = g.elapsed_seconds + dt * f64::from(g.time_scale);
    g.set_elapsed(t);
}

/// FarmingSystem's milliseconds a frame at 30 fps with the showcase-size garden: the average, the
/// median and the worst over ten seconds of real-time play, after a second to settle in.
/// `settings` publishes the farm's interval when given (rung 2), else the DataStore has none.
fn measure(interval_s: Option<f64>) -> (f64, f64, f64) {
    let (mut data, mut world) = showcase_size_garden();
    if let Some(s) = interval_s {
        data.insert(super::clock::INTERVAL_KEY, Mutex::new(s));
    }
    let mut sys = FarmingSystem::new();
    // The reuse switch on, whatever the process's (crate::reuse is one switch for every test).
    sys.reuse_override = Some(true);
    let dt = 1.0 / FPS;
    for _ in 0..FPS as usize {
        advance_clock(&data, dt);
        sys.tick(&mut world, dt as f32, &data);
    }
    let mut ms: Vec<f64> = Vec::new();
    for _ in 0..(10.0 * FPS) as usize {
        advance_clock(&data, dt);
        let t0 = std::time::Instant::now();
        sys.tick(&mut world, dt as f32, &data);
        ms.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    let avg = ms.iter().sum::<f64>() / ms.len() as f64;
    let worst = ms.iter().copied().fold(0.0, f64::max);
    ms.sort_by(f64::total_cmp);
    (avg, ms[ms.len() / 2], worst)
}

/// THE COST, printed (see the module comment): every frame (no interval published, as before rung
/// 2), and with rung 2's interval at every second and every minute.
#[test]
#[ignore]
fn farm_cost_per_frame() {
    for (label, interval) in [("no interval published (every frame)", None), ("every second", Some(1.0)), ("every minute", Some(60.0))] {
        let (avg, median, worst) = measure(interval);
        println!("FarmingSystem, {CROPS} crops in {AREAS} areas at {FPS} fps, {label}: average {avg:.3} ms, median {median:.3} ms, worst {worst:.3} ms");
    }
}

// -- The rotation's rule -------------------------------------------------------------------------

/// A frame brings up the turns whose start it crosses, the air on slot 0 or its own second;
/// every turn exactly once an interval; everything for a frame that spans an interval or with
/// `every`; nothing for a frame of no game time.
#[test]
fn a_frame_brings_up_the_turns_it_crosses() {
    use super::clock::{plan, TURNS};
    let up = |d: &super::clock::Due| (0..TURNS as usize).filter(|k| d.turns[*k]).collect::<Vec<_>>();
    // Every frame (the interval 0), or `every` (reuse off, a burn): everything, the air too.
    assert!(plan(9.9, 10.0, 0.0, 0.0, false).all && plan(9.9, 10.0, 0.0, 0.0, false).air);
    assert!(plan(9.9, 10.0, 3600.0, 1.0, true).all && plan(9.9, 10.0, 3600.0, 1.0, true).air);
    // A second cut in TURNS: a frame from 0.8 to 1.2 of a turn crosses turn 1's start.
    let b = 1.0 / f64::from(TURNS);
    let d = plan(0.8 * b, 1.2 * b, 1.0, 1.0, false);
    assert!(!d.all && !d.air && up(&d) == vec![1], "{d:?}");
    // One crossing a whole second brings up slot 0, the air's.
    let d = plan(0.98, 1.01, 1.0, 1.0, false);
    assert!(up(&d) == vec![0] && d.air, "{d:?}");
    // A held clock brings up nothing; a frame longer than the interval, everything.
    let d = plan(5.0, 5.0, 1.0, 1.0, false);
    assert!(!d.all && !d.air && up(&d).is_empty());
    let d = plan(8.0, 10.0, 1.0, 1.0, false);
    assert!(d.all && d.air);
    // An hour's interval at 30 frames a second, the clock summed frame by frame as the farm sums
    // it: in an hour every turn comes up exactly once, and the air every second. Seen red
    // 2026-10-31 with the frame's start worked back as `t - dt`: turn 1 came up twice and the air
    // 3,876 times, each where that subtraction rounded below a boundary already crossed.
    let (mut counts, mut air) = ([0u32; TURNS as usize], 0u32);
    let dt = 1.0 / 30.0;
    let mut t = 0.0_f64;
    // A frame past the hour, so the summed clock (a hair under 3,600 s after 108,000 frames of
    // 1/30 s) crosses the hour's last boundary.
    for _ in 0..=(3600 * 30) {
        let t0 = t;
        t += dt;
        let d = plan(t0, t, 3600.0, 1.0, false);
        assert!(!d.all);
        for t in up(&d) {
            counts[t] += 1;
        }
        air += u32::from(d.air);
    }
    assert_eq!(counts, [1; TURNS as usize], "each turn once an hour");
    assert_eq!(air, 3600, "the air every second");
}

/// A new area joins the turn holding the fewest crops, never slot 0 (the air's), and a frame's
/// due areas are grouped by what they owe.
#[test]
fn new_areas_balance_the_turns_and_a_turn_is_one_group() {
    let mut c = super::clock::FarmClock::new();
    let a = c.index_of("tower", 0.0, 50);
    let b = c.index_of("bed", 0.0, 10);
    let d = c.index_of("field", 0.0, 10);
    let e = c.index_of("bed2", 0.0, 10);
    let turns: Vec<u32> = [a, b, d, e].iter().map(|i| c.areas[*i as usize].turn).collect();
    assert!(turns.iter().all(|t| *t >= 1 && *t < super::clock::TURNS), "{turns:?}");
    assert_eq!(turns[..3], [1, 2, 3], "each new area on the emptiest turn");
    assert_eq!(c.index_of("tower", 9.0, 0), a, "a known area keeps its place and clock");
    c.pass(0.5);
    c.areas[b as usize].owed = 0.0;
    let groups = c.groups(&[a, b, d]);
    assert_eq!(groups.len(), 2, "two different debts, two runs: {groups:?}");
    assert_eq!(groups[0], (0.5, vec![a, d]), "the longest debt first");
}

/// EACH AREA ADVANCES ONCE AN INTERVAL, no more: the showcase-size garden (2,000 crops in 90
/// areas) at 30 frames a second with the interval at every second, ten seconds after it settles
/// in. Every area's clock goes back to zero ten times, once on its turn each second, and no turn
/// holds more than its share of the crops (the turns are balanced by crops as areas are seen).
#[test]
fn each_area_advances_once_an_interval() {
    let (mut data, mut world) = showcase_size_garden();
    data.insert(super::clock::INTERVAL_KEY, Mutex::new(1.0));
    let mut sys = FarmingSystem::new();
    sys.reuse_override = Some(true);
    let dt = 1.0 / FPS;
    for _ in 0..FPS as usize {
        advance_clock(&data, dt);
        sys.tick(&mut world, dt as f32, &data);
    }
    let mut advances = vec![0u32; sys.clock.areas.len()];
    for _ in 0..(10.0 * FPS) as usize {
        advance_clock(&data, dt);
        sys.tick(&mut world, dt as f32, &data);
        for (i, a) in sys.clock.areas.iter().enumerate() {
            advances[i] += u32::from(a.owed == 0.0);
        }
    }
    assert_eq!(sys.clock.areas.len(), AREAS);
    for (i, n) in advances.iter().enumerate() {
        assert!((9..=11).contains(n), "{} advanced {n} times in ten seconds", sys.clock.areas[i].name);
    }
    let mut per_turn = [0usize; super::clock::TURNS as usize];
    for a in &sys.clock.areas {
        per_turn[a.turn as usize] += a.crops;
    }
    assert_eq!(per_turn[0], 0, "slot 0 is the air's alone");
    let share = CROPS as f64 / f64::from(super::clock::TURNS - 1);
    let most = per_turn.iter().copied().max().unwrap_or(0) as f64;
    assert!(most <= share * 1.25, "the heaviest turn holds {most} crops against a share of {share:.0}: {per_turn:?}");
}

// -- Helpers for the garden runs -------------------------------------------------------------------

/// Put the clock at `t` game seconds.
fn set_clock(data: &DataStore, t: f64) {
    data.get::<Mutex<crate::systems::time::GameTime>>("game_time").unwrap().lock().unwrap().set_elapsed(t);
}

/// The step_tests garden (every regime crossed) moved to start at `start_s` with every crop's age
/// kept, run `span_s` game seconds in frames of `frame_s`, after step_tests' one-second warm-up,
/// with the farm's interval published as given (None: every frame, the farm as it ran before
/// rung 2) and the reuse switch as given. Then the save's ask (`settle(.., 0)`) brings every area
/// and the air to now, and the end is read.
fn run_clocked(start_s: f64, span_s: f64, frame_s: f64, interval_s: Option<f64>, air: bool, reuse: bool) -> (step_tests::Ended, FarmingSystem) {
    let (mut data, mut world) = step_tests::garden(air);
    let shift = start_s - step_tests::START_S;
    for (_e, c) in world.query_mut::<&mut CropInstance>() {
        c.planted_at += shift;
    }
    set_clock(&data, start_s);
    if let Some(s) = interval_s {
        data.insert(super::clock::INTERVAL_KEY, Mutex::new(s));
    }
    let mut sys = FarmingSystem::new();
    sys.reuse_override = Some(reuse);
    set_clock(&data, start_s + step_tests::WARM_S);
    sys.tick(&mut world, step_tests::WARM_S as f32, &data);
    let n = (span_s / frame_s).round() as usize;
    for k in 1..=n {
        set_clock(&data, start_s + step_tests::WARM_S + k as f64 * frame_s);
        sys.tick(&mut world, frame_s as f32, &data);
    }
    sys.settle(&mut world, &data, 0.0);
    (step_tests::ended(&world), sys)
}

/// Compare `other` with `base` and say how far apart they are; the failures against the bound
/// step_tests.rs set for a step of `step_s` game seconds.
fn held_to_bounds(label: &str, base: &step_tests::Ended, other: &step_tests::Ended, air: bool, step_s: f64, frames: usize, sunrise: bool) -> Vec<String> {
    let (stages, worst) = step_tests::compare(base, other);
    println!("--- {label}: {} stage mismatches", stages.len());
    for s in stages.iter().take(8) {
        println!("    stage: {s}");
    }
    for (k, w) in &worst {
        if w.abs > 0.0 {
            println!("    {k}: max abs {:.3e} (rel {:.3e}) at {}", w.abs, w.rel, w.at);
        }
    }
    let mut failures = Vec::new();
    if !stages.is_empty() {
        failures.push(format!("{label}: {} crops ended in a different stage, first {}", stages.len(), stages[0]));
    }
    for (k, w) in &worst {
        let (abs_tol, rel_tol) = tolerance(k, air, step_s, frames, sunrise);
        if w.abs > abs_tol && w.rel > rel_tol {
            failures.push(format!("{label}: {k} differs by {:.3e} (rel {:.3e}) at {}, over {abs_tol:e} / {rel_tol:e}", w.abs, w.rel, w.at));
        }
    }
    failures
}

/// The bound each quantity is held to against the every-frame farm: step_tests.rs's for a step of
/// the interval (`step_s`), and what comparing with a reference of `frames` frames adds to it, each
/// measured (2026-10-31) and each said:
/// - The reference itself stores each crop's water and health as an f32 every frame. At a tenth
///   of a second a frame's change is a few tens of ulps, so its rounding runs the same way frame
///   after frame: up to a 2^-24 a frame for the water (under 1) and a 2^-17 for the health (under
///   128). Measured at 1,500 frames: water 3.6e-5 and health 2.3e-3 apart, the farm on its own
///   clock the nearer to the exact path (it rounds once a turn).
/// - The air's water ledger (litres breathed, condensed, leaked, lying on surfaces) is a running
///   sum of each air step's litres; a millionth of a litre is nothing a tank shows.
/// - A pest's level a few millionths of a full infestation: the pests read each crop's water stress
///   at the step's start (rung 1's own first-order term), so where an area's turn falls against
///   bed_b's drying moves them. Measured at an hour: aphids 1.2e-6 apart at 3.6e-5.
/// - `sunrise` (the short run, which ends 75 s after a sunrise): the rooms' air is in its fastest
///   change of the day, the crops starting to draw its carbon dioxide down, and the air steps on
///   each area's breath as that area's last turn worked it out, so its drawdown starts up to an
///   interval late; the air's own step (a second, against the reference's tenth) adds a first
///   order term of its own. Measured: the rooms' and the home's air 1.6e-3 apart at a second and
///   15% at a minute (a room's CO2), a mushroom's CO2-capped health 0.19 at a minute. Over thirty
///   hours (`ten_minutes_and_an_hour_...`) those meet rung 1's own bounds.
fn tolerance(key: &str, air: bool, step_s: f64, frames: usize, sunrise: bool) -> (f64, f64) {
    let (abs, rel) = step_tests::tolerance(key, air, step_s);
    let n = frames as f64;
    let air_now = air && sunrise;
    match key {
        "crop.water" => (abs.max(n * 2f64.powi(-24)), rel),
        "crop.health" if air_now => (abs.max(n * 2f64.powi(-17) + 4e-3 * step_s), rel),
        "crop.health" => (abs.max(n * 2f64.powi(-17)), rel),
        "crop.health_seconds" if air_now => (abs.max(2e-3 * step_s), rel),
        k if air_now && (k.starts_with("memory.home_air.") || k.starts_with("memory.rooms.")) => {
            let floor = if k.ends_with("_l") { 1e-6 } else { abs };
            (floor, rel.max(4e-3 + 3e-3 * step_s))
        }
        k if k.starts_with("memory.home_air.") && k.ends_with("_l") => (abs.max(1e-6), rel),
        "memory.pests.level" => (abs.max(2e-9 * step_s), rel),
        _ => (abs, rel),
    }
}

// -- The switch ------------------------------------------------------------------------------------

/// THE SWITCH (CLAUDE.md, the reuse rule): an area not yet advanced to now is state kept stale on
/// purpose, so with the Dev page's "Turn off all reuse" off every area and the air advance every
/// frame, whatever the interval, and the garden ends to the bit where the farm with no interval
/// ends. With reuse on, the same hour's interval leaves areas behind between their turns (so the
/// check can fail).
#[test]
fn with_reuse_off_every_area_advances_every_frame() {
    for reuse in [false, true] {
        let (mut data, mut world) = step_tests::garden(true);
        data.insert(super::clock::INTERVAL_KEY, Mutex::new(3600.0));
        let mut sys = FarmingSystem::new();
        sys.reuse_override = Some(reuse);
        let mut behind = 0usize;
        for k in 1..=60 {
            set_clock(&data, step_tests::START_S + f64::from(k) * 10.0);
            sys.tick(&mut world, 10.0, &data);
            behind += sys.clock.areas.iter().filter(|a| a.owed != 0.0).count() + usize::from(sys.clock.air_owed != 0.0);
        }
        assert!(!sys.clock.areas.is_empty());
        if reuse {
            assert!(behind > 0, "with reuse on, an hour's interval leaves areas between their turns");
        } else {
            assert_eq!(behind, 0, "with reuse off, every area and the air advanced every frame");
        }
    }
    // To the bit: reuse off at an hour's interval against no interval at all, ten minutes of 10 s.
    let (off, _) = run_clocked(step_tests::START_S, 600.0, 10.0, Some(3600.0), true, false);
    let (none, _) = run_clocked(step_tests::START_S, 600.0, 10.0, None, true, true);
    let (stages, worst) = step_tests::compare(&none, &off);
    assert!(stages.is_empty(), "{stages:?}");
    for (k, w) in &worst {
        assert_eq!(w.abs, 0.0, "{k} differs with reuse off: {w:?}");
    }
}

// -- Equivalence -----------------------------------------------------------------------------------

/// EVERY SECOND AND EVERY MINUTE END WHERE EVERY FRAME ENDS: two and a half game minutes across a
/// sunrise in frames of a tenth of a second (so a second's interval is ten frames and its eight
/// turns land on different frames), with and without the air, held to step_tests' bounds for a
/// step of the interval.
#[test]
fn every_second_and_every_minute_end_where_every_frame_ends() {
    let start = 5.0 * 3600.0 + 58.0 * 60.0 + 45.0;
    let mut failures = Vec::new();
    for air in [false, true] {
        let (base, _) = run_clocked(start, 150.0, 0.1, None, air, true);
        for interval in [1.0, 60.0] {
            let (other, sys) = run_clocked(start, 150.0, 0.1, Some(interval), air, true);
            assert!(sys.clock.areas.iter().all(|a| a.owed == 0.0) && sys.clock.air_owed == 0.0, "the save's ask left nothing behind");
            failures.extend(held_to_bounds(&format!("{} air, every {interval} s", if air { "with" } else { "without" }), &base, &other, air, interval, 1_500, true));
        }
    }
    assert!(failures.is_empty(), "the farm's own clock moved the garden's end:\n{}", failures.join("\n"));
}

/// The thirty game hours of step_tests at every frame of a minute, and at each of `intervals`:
/// what each ends at against the every-frame farm, and the failures against `tolerance`.
fn thirty_hours_at(intervals: &[f64]) -> Vec<String> {
    let frames = (step_tests::SPAN_S / step_tests::SHORT_S) as usize;
    let mut failures = Vec::new();
    for air in [false, true] {
        let (base, _) = run_clocked(step_tests::START_S, step_tests::SPAN_S, step_tests::SHORT_S, None, air, true);
        let (_, w0) = step_tests::garden(air);
        let start: Vec<String> = {
            let mut v: Vec<(u64, String)> = w0.query::<&CropInstance>().iter().map(|(e, c)| (e.to_bits().get(), c.growth_stage.clone())).collect();
            v.sort_by_key(|(id, _)| *id);
            v.into_iter().map(|(_, s)| s).collect()
        };
        let (dead, moved, below) = step_tests::activity(&base, &start);
        println!("{} air, every frame of {} s: {dead} died, {moved} changed stage, {below} living below 99 health", if air { "with" } else { "without" }, step_tests::SHORT_S);
        assert!(dead > 0 && moved > 0 && below > 0, "the garden crossed its regimes");
        for interval in intervals {
            let (other, _) = run_clocked(step_tests::START_S, step_tests::SPAN_S, step_tests::SHORT_S, Some(*interval), air, true);
            failures.extend(held_to_bounds(&format!("{} air, every {interval} s", if air { "with" } else { "without" }), &base, &other, air, *interval, frames, false));
        }
    }
    failures
}

/// TEN MINUTES AND AN HOUR END WHERE EVERY FRAME ENDS: step_tests' thirty game hours (two
/// sunrises, a sunset, the lamps' switch-off, a bed drying out and dying, crops turning stage and
/// ripening, the feeder feeding) in frames of a game minute, with and without the air, held to
/// step_tests' bounds for a step of the interval: without the air every step is exact to rounding;
/// with it the rooms' air and the crops' caps meet a turn apart, first order in the interval.
#[test]
fn ten_minutes_and_an_hour_end_where_every_frame_ends() {
    let failures = thirty_hours_at(&[600.0, 3_600.0]);
    assert!(failures.is_empty(), "the farm's own clock moved the garden's end:\n{}", failures.join("\n"));
}

/// SIX HOURS AND A DAY, MEASURED: what the two longer intervals the operator named do to the same
/// thirty hours (farm-clock.md says why neither is offered yet). A measurement, not a check:
/// `just test-lib six_hours_and_a_day_measured --ignored --nocapture`.
#[test]
#[ignore]
fn six_hours_and_a_day_measured() {
    for f in thirty_hours_at(&[21_600.0, 86_400.0]) {
        println!("over rung 1's bound: {f}");
    }
}

// -- The readers -----------------------------------------------------------------------------------

/// A HARVEST BETWEEN TWO TURNS MEETS THE CROP AS IT IS NOW. A lettuce in a bed on an hour's
/// interval ripens ten minutes after its area's turn; twenty minutes on, its area has not had
/// another, so the crop in the world still reads its old stage. The harvest pressed then must
/// bring the area up to now first, find it ripe and pick it. Seen red here, every run, with the
/// tick's bring-up of what a command touches left out (`no_bring_up`): the press finds an unripe
/// lettuce and does nothing.
#[test]
fn a_harvest_between_turns_meets_the_crop_as_it_is_now() {
    assert_eq!(harvest_between_turns(false), (true, true), "brought up to now, the harvest picks the ripe lettuce");
    assert_eq!(harvest_between_turns(true), (false, false), "without the bring-up the press meets the crop as its last turn left it");
}

/// The garden of `a_harvest_between_turns_meets_the_crop_as_it_is_now`, the harvest pressed
/// between turns, with or without the bring-up: (the crop is gone, a lettuce is in the backpack).
fn harvest_between_turns(no_bring_up: bool) -> (bool, bool) {
    let mut data = gardening_tests::make_store();
    data.insert(super::clock::INTERVAL_KEY, Mutex::new(3600.0));
    data.insert("player_notices", Mutex::new(Vec::<String>::new()));
    {
        let gt = data.get::<Mutex<crate::systems::time::GameTime>>("game_time").unwrap();
        let mut g = gt.lock().unwrap();
        g.time_scale = 1.0;
        g.set_elapsed(11.0 * 3600.0);
    }
    let reg = data.get::<PlantRegistry>("plant_registry").unwrap();
    let def = reg.get("lettuce").expect("lettuce in plants.csv");
    let growth_s = f64::from(def.growth_days) * SECONDS_PER_DAY;
    let stages = def.stages();
    let mut world = hecs::World::new();
    world.spawn((crate::systems::inventory::Inventory::new(40), crate::ecs::components::Controllable));
    // Under the sun the growth clock runs at two seconds a second: ripe 1,080 s from now, after
    // the bed's first turn (slot 1 of the hour's turns, which starts within its first 8 minutes).
    let t0 = 11.0 * 3600.0;
    let crop = world.spawn((CropInstance {
        crop_def_id: "lettuce".to_string(),
        growth_stage: stages[stages.len() - 2].to_string(),
        planted_at: t0 - (growth_s - 2.0 * 1_080.0),
        water_level: 1.0,
        health: 100.0,
        tower_id: Some("lettuce_bed_1".to_string()),
        tower_slot: Some(0),
        health_seconds: 0.0,
        growing_seconds: 0.0,
    },));
    let mut sys = FarmingSystem::new();
    sys.reuse_override = Some(true);
    sys.no_bring_up = no_bring_up;
    let mut t = t0;
    let mut frame = |sys: &mut FarmingSystem, world: &mut hecs::World| {
        t += 60.0;
        set_clock(&data, t);
        sys.tick(world, 60.0, &data);
    };
    // The first frame advances the new bed at once (`AreaClock::fresh`); its turn comes later.
    frame(&mut sys, &mut world);
    let area = sys.clock.find("lettuce_bed_1").expect("the bed has a clock") as usize;
    let mut had_turn = false;
    for _ in 1..8 {
        frame(&mut sys, &mut world);
        had_turn |= sys.clock.areas[area].owed == 0.0;
    }
    assert!(had_turn, "the bed had its turn within the first 8 minutes");
    for _ in 0..17 {
        frame(&mut sys, &mut world);
    }
    assert!(sys.clock.areas[area].owed > 1_000.0, "the bed waits for its next turn");
    let stage_now = world.get::<&CropInstance>(crop).unwrap().growth_stage.clone();
    assert_ne!(stage_now, def.last_stage(), "between turns the world still holds the crop as its last turn left it");
    *data.get::<Mutex<Option<u64>>>("harvest_request").unwrap().lock().unwrap() = Some(crop.to_bits().get());
    frame(&mut sys, &mut world);
    let gone = world.get::<&CropInstance>(crop).is_err();
    let picked: u32 = world.query::<&crate::systems::inventory::Inventory>().iter().map(|(_, inv)| inv.count_item("vegetable_lettuce_0")).sum();
    (gone, picked > 0)
}

/// A READER THAT LOOKS KEEPS THE GARDEN WITHIN A SECOND, without catching it all up every frame.
/// With an hour's interval, the Garden page's (or the plant meshes') first ask brings every area
/// and the air within a second of now; while it keeps asking, the farm's turns run on a second and
/// no area falls more than a second and a turn behind, so its later asks find nothing to do.
#[test]
fn a_reader_that_looks_keeps_the_garden_within_a_second() {
    let (mut data, mut world) = step_tests::garden(true);
    data.insert(super::clock::INTERVAL_KEY, Mutex::new(3600.0));
    let mut sys = FarmingSystem::new();
    sys.reuse_override = Some(true);
    let mut t = step_tests::START_S;
    for _ in 0..100 {
        t += 0.25;
        set_clock(&data, t);
        sys.tick(&mut world, 0.25, &data);
    }
    let worst = |sys: &FarmingSystem| sys.clock.areas.iter().map(|a| a.owed).fold(sys.clock.air_owed, f64::max);
    assert!(worst(&sys) > 20.0, "unlooked, an hour's interval leaves areas behind: {}", worst(&sys));
    let limit = super::clock::LOOK_FRESH_S * (1.0 + 1.0 / f64::from(super::clock::TURNS));
    sys.settle(&mut world, &data, super::clock::LOOK_FRESH_S);
    assert!(worst(&sys) <= limit, "the first ask brought the garden within a second: {}", worst(&sys));
    for _ in 0..100 {
        t += 0.25;
        set_clock(&data, t);
        sys.tick(&mut world, 0.25, &data);
        // The reader runs after the tick in the frame, as the Garden page's data does.
        assert!(worst(&sys) <= limit + 0.25, "while it looks, no area is more than a second and a turn behind: {}", worst(&sys));
        sys.settle(&mut world, &data, super::clock::LOOK_FRESH_S);
    }
}

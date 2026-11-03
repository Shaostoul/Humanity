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

use std::sync::Mutex;

use super::*;
use super::fixtures::step::*;
use humanity_core::ecs::components::CropInstance;
use humanity_core::ecs::systems::System;

/// The reference tick, game seconds.
pub(super) const SHORT_S: f64 = 60.0;
/// How long each run is: a day and a quarter, two sunrises, a sunset and the lamps' switch-off.
pub(super) const SPAN_S: f64 = 30.0 * 3600.0;
/// The longer ticks, as multiples of the reference one.
const MULTIPLES: [u32; 2] = [10, 60];

/// The garden run SPAN_S game seconds at ticks of `step_s`, after the warm-up tick.
fn run(step_s: f64, air: bool) -> Ended {
    let (data, mut world) = garden(air);
    let mut sys = FarmingSystem::new();
    let set_clock = |t: f64| data.get::<Mutex<crate::time::GameTime>>("game_time").unwrap().lock().unwrap().set_elapsed(t);
    set_clock(START_S + WARM_S);
    sys.tick(&mut world, WARM_S as f32, &data);
    let n = (SPAN_S / step_s).round() as usize;
    for k in 1..=n {
        set_clock(START_S + WARM_S + k as f64 * step_s);
        sys.tick(&mut world, step_s as f32, &data);
    }
    ended(&world)
}

/// What a run did, so a pass is not a pass of an inert garden: crops that died, that changed
/// stage, and living crops below 99 health.
pub(super) fn activity(r: &Ended, start: &[String]) -> (usize, usize, usize) {
    let dead = r.0.iter().filter(|c| c.stage == humanity_core::ecs::components::STAGE_DEAD).count();
    let moved = r.0.iter().zip(start).filter(|(c, s)| c.stage != **s && c.stage != humanity_core::ecs::components::STAGE_DEAD).count();
    let below = r.0.iter().filter(|c| c.health < 99.0 && c.stage != humanity_core::ecs::components::STAGE_DEAD).count();
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
pub(super) fn tolerance(key: &str, air: bool, step_s: f64) -> (f64, f64) {
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

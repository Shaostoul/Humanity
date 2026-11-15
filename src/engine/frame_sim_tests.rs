//! The systems slice stepped with no GPU (the headless sim runner's increment 2, 2026-11-15;
//! docs/design/headless-sim-runner.md): the homestead entered as the windowed game enters it
//! (engine/sim_home.rs: sim_boot, the registries, the save's progress, world_sim::build), then
//! `frame_sim::systems_slice`, the step the windowed frame runs, driven with no wgpu Instance, no
//! window and no `EngineState`.
use crate::engine::sim_home;
use crate::engine::sim_soak::{self, StepTolerances, DAY_S};

/// The game's data folder, as the windowed game finds it.
fn data_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("HUMANITY_REPO_ROOT")).join("data")
}

/// A saves folder of this test's own, empty, so no player's save is read or written.
fn scratch_saves(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("hos-frame-sim-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch saves folder");
    dir
}

fn tolerances() -> StepTolerances {
    StepTolerances::load(&data_dir()).expect("data/sim/step_tolerances.ron")
}

fn get(obs: &sim_soak::Obs, id: &str) -> f64 {
    *obs.get(id).unwrap_or_else(|| panic!("no reading of {id}: {obs:?}"))
}

/// A game day of the homestead, stepped at the safe step with no GPU: what a day must change.
///
/// Seen red 2026-11-15, before it was trusted: its first run entered the homestead with the
/// GuiState's default page still over the world, so `world_hold::tick` held the systems (BUG-253)
/// and it failed "the clock moved 0 s in a game day" (every reading unchanged: hydration 80 -> 80,
/// the batteries 16000 -> 16000). `sim_home::enter` now steps into the world as the game's
/// showroom_confirm does.
#[test]
fn a_game_day_passes_with_no_gpu() {
    let tol = tolerances();
    let mut home = sim_home::enter(&data_dir(), scratch_saves("day"));
    let planted = home.plant_first_tower(&data_dir());
    assert!(planted.is_some(), "no tower design to plant");
    let before = home.observe();
    let steps = (DAY_S / tol.safe_step_s).round() as usize;
    for _ in 0..steps {
        home.step(tol.safe_step_s as f32);
    }
    let after = home.observe();
    let moved = |id: &str| get(&after, id) - get(&before, id);

    // The clock: a day on.
    assert!((moved("clock.elapsed_s") - DAY_S).abs() < 1e-3, "the clock moved {} s in a game day", moved("clock.elapsed_s"));
    // The body: a day without a meal or a drink takes fullness and water down, and rest; the
    // person is still alive and well (a day is far from starving).
    assert!(moved("vitals.satiation") < -1.0, "fullness moved {} in a day with no meal", moved("vitals.satiation"));
    assert!(moved("vitals.hydration") < -10.0, "hydration moved {} in a day with no drink", moved("vitals.hydration"));
    assert!(moved("vitals.energy") < -10.0, "rest moved {} in a day awake", moved("vitals.energy"));
    assert!(moved("vitals.waste") > 1.0, "waste moved {} in a day", moved("vitals.waste"));
    assert!(get(&after, "body.health") > 50.0, "the body's health fell to {} in one day", get(&after, "body.health"));
    // The garden: the tower planted on the first step, every crop growing all day.
    let crops = get(&after, "crops.count");
    assert!(crops >= 1.0, "the tower planted no crop (the backpack's seeds)");
    assert!(
        get(&after, "crops.growing_s") > 0.9 * crops * DAY_S,
        "the {crops} crops grew {} s between them in a day",
        get(&after, "crops.growing_s")
    );
    // The machines: the batteries and the tanks are live numbers, moved by a day of sun, load,
    // purifier and household.
    assert!(moved("power.battery_wh").abs() > 1.0, "the batteries did not move in a day ({} Wh)", get(&after, "power.battery_wh"));
    assert!(moved("water.tank_l").abs() > 1.0, "the tanks did not move in a day ({} L)", get(&after, "water.tank_l"));
    // The home's air is read (the sealed home's own air space).
    assert!(after.contains_key("air.co2_g_m3"), "the home's air is not read: {after:?}");
}

/// The split-step test: one span stepped at the windowed frame's dt and at coarse steps lands
/// every quantity within its stated tolerance (data/sim/step_tolerances.ron, as a share of how
/// far it moved plus a floor in its unit).
///
/// Seen red 2026-11-15, on a real system that could not coarse-step: the batteries
/// (ElectricalSystem) at 10 s off by 7.79 Wh (allowed 7.11) and at 60 s off by 51.3 Wh (allowed
/// 7.11), 15906.5 against 15957.8 at the frame's dt. Every load the air step drives spawned at its
/// full draw and the electrical sim integrates a step before the air step writes the draw, so the
/// first step billed the seven air handlers and the recycler at 3.1 kW for the whole step (1/60 s
/// in the windowed game, a minute here). BUG-314: they start idle now (engine/home_spawn.rs).
/// It also caught the planting landing on a step boundary (each crop a step's growth short),
/// which was the run's script, not the farm: sim_soak.rs plants with a step of no time.
#[test]
fn a_span_stepped_at_the_frames_dt_and_coarsely_agrees() {
    let tol = tolerances();
    let span_s = 120.0;
    let frame_dt = 1.0 / 60.0;
    let reference = sim_soak::run_span(&data_dir(), tol.seed, frame_dt, span_s, None);
    let mut off = Vec::new();
    for step_s in [1.0, 10.0, 60.0] {
        let run = sim_soak::run_span(&data_dir(), tol.seed, step_s, span_s, None);
        for v in sim_soak::judge(&tol, &reference.first, &reference.end, &run.end) {
            if !v.holds() {
                off.push(format!("{} ({}) at {step_s} s off by {} (allowed {}): {} against {}", v.id, v.system, v.error, v.allowed, v.coarse, v.reference));
            }
        }
    }
    assert!(off.is_empty(), "{span_s} game seconds at the frame's {frame_dt:.4} s step against coarse steps:\n  {}", off.join("\n  "));
}

/// The step ladder (the design's section 3): a span at the reference step against every rung,
/// which systems hold at which step printed, and the recorded safe step must still hold (every
/// rung up to it), so the soak never steps coarser than measured.
///
/// Seen red 2026-11-15, its first run: "crops.growing_s (FarmingSystem) at 10 s off by 36
/// (allowed 1)" and at 60 s by 236 (the tower planted by the first real step, each crop a step's
/// growth short; sim_soak.rs now plants with a step of no time). Before BUG-314's fix the ladder
/// also found the batteries off at 600 s, 521.6 Wh against 128.8 allowed, the first step's
/// nameplate billing of the air handlers (3.1 kW for 600 s); with it fixed every rung holds over
/// these two night hours, and the day's sun is the 24-hour ladder's (`just sim-ladder`).
#[test]
fn the_recorded_safe_step_holds_on_the_ladder() {
    let tol = tolerances();
    let span_s = 2.0 * 3600.0;
    let (reference, rungs) = sim_soak::ladder(&data_dir(), &tol, span_s, &tol.ladder_s);
    let text = sim_soak::ladder_text(&tol, span_s, &reference, &rungs);
    eprintln!("{text}");
    let failing: Vec<String> = rungs
        .iter()
        .filter(|r| r.step_s <= tol.safe_step_s && !r.holds())
        .flat_map(|r| r.verdicts.iter().filter(|v| !v.holds()).map(move |v| format!("{} ({}) at {} s off by {} (allowed {})", v.id, v.system, r.step_s, v.error, v.allowed)))
        .collect();
    assert!(
        failing.is_empty(),
        "the recorded safe step, {} s, no longer holds (data/{}):\n  {}\n{text}",
        tol.safe_step_s,
        sim_soak::TOLERANCES_FILE,
        failing.join("\n  ")
    );
    assert!(tol.ladder_s.iter().any(|s| (s - tol.safe_step_s).abs() < 1e-9), "the safe step is not a rung of the ladder");
}

/// The same seed and the same steps give the same numbers, run after run (the design's
/// "byte-identical repeat runs", for what the homestead's day exercises; sim_seed.rs says which
/// random draws are not seeded yet). A whole day, through the sun's rise and set.
///
/// Seen red 2026-11-15 with a clock's nanoseconds added to the batteries' reading (a draw nobody
/// seeded reaching a quantity): "two runs of one seed differ: power.battery_wh: 28525.2370691572
/// then Some(28525.2323810702)". (With the run's seed left out it still passed: the homestead
/// aboard sees no weather, so the seed's streams do not reach its quantities; a planet run's will.)
#[test]
fn the_same_run_repeats_exactly() {
    let tol = tolerances();
    let a = sim_soak::run_span(&data_dir(), tol.seed, 60.0, DAY_S, None);
    let b = sim_soak::run_span(&data_dir(), tol.seed, 60.0, DAY_S, None);
    let differ: Vec<String> = a
        .end
        .iter()
        .filter(|(k, v)| b.end.get(*k).map(|w| w.to_bits()) != Some(v.to_bits()))
        .map(|(k, v)| format!("{k}: {v} then {:?}", b.end.get(k)))
        .collect();
    assert!(differ.is_empty(), "two runs of one seed differ:\n  {}", differ.join("\n  "));
}

/// Every quantity has its tolerance row and every row names a quantity.
#[test]
fn tolerances_cover_every_observable() {
    let tol = tolerances();
    let ids: Vec<&str> = sim_home::OBSERVABLES.iter().map(|o| o.id).collect();
    for id in &ids {
        assert!(tol.observables.iter().any(|t| t.id == *id), "{id} has no row in data/{}", sim_soak::TOLERANCES_FILE);
    }
    for t in &tol.observables {
        assert!(ids.contains(&t.id.as_str()), "data/{} names {}, which nothing reads", sim_soak::TOLERANCES_FILE, t.id);
        assert!(t.abs >= 0.0 && t.rel >= 0.0 && !t.why.trim().is_empty(), "{}: a tolerance is a non-negative number with its why", t.id);
    }
    assert!(tol.reference_step_s > 0.0 && tol.safe_step_s >= tol.reference_step_s, "the safe step is coarser than the reference");
}

/// A soak refuses a step coarser than the safe one unless forced (the design's risk 6), before
/// it steps anything.
#[test]
fn a_step_past_the_safe_one_is_refused_unless_forced() {
    let tol = tolerances();
    let ask = sim_soak::SoakAsk { days: 1.0, step_s: Some(tol.safe_step_s * 10.0), force: false, check_hours: None, seed: None };
    let err = sim_soak::soak(&data_dir(), &ask).expect_err("a step past the safe one ran");
    assert!(err.contains("safe step") && err.contains("--force"), "the refusal does not say why or how: {err}");
}


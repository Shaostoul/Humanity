//! The systems slice stepped with no GPU (the headless sim runner's increment 2, 2026-11-15;
//! docs/design/headless-sim-runner.md): the homestead entered as the windowed game enters it
//! (engine/sim_home.rs: sim_boot, the registries, the save's progress, world_sim::build), then
//! `frame_sim::systems_slice`, the step the windowed frame runs, driven with no wgpu Instance, no
//! window and no `EngineState`.
use crate::engine::sim_home;
use crate::engine::sim_soak::{self, StepTolerances, DAY_S};
use crate::engine::sim_routine;

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
    let reference = sim_soak::run_span(&data_dir(), tol.seed, frame_dt, span_s, None, None);
    let mut off = Vec::new();
    for step_s in [1.0, 10.0, 60.0] {
        let run = sim_soak::run_span(&data_dir(), tol.seed, step_s, span_s, None, None);
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
    let (reference, rungs) = sim_soak::ladder(&data_dir(), &tol, span_s, &tol.ladder_s, None);
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
    let a = sim_soak::run_span(&data_dir(), tol.seed, 60.0, DAY_S, None, None);
    let b = sim_soak::run_span(&data_dir(), tol.seed, 60.0, DAY_S, None, None);
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
    let ask = sim_soak::SoakAsk { days: 1.0, step_s: Some(tol.safe_step_s * 10.0), force: false, check_hours: None, seed: None, routine: None };
    let err = sim_soak::soak(&data_dir(), &ask).expect_err("a step past the safe one ran");
    assert!(err.contains("safe step") && err.contains("--force"), "the refusal does not say why or how: {err}");
}

// -- Increment 3: the frame's bridges on the sim's parts, and a person's routine -------------------

/// The daily routine (data/sim/routines/daily.ron).
fn daily() -> sim_routine::Routine {
    sim_routine::Routine::load(&data_dir(), "daily").expect("data/sim/routines/daily.ron")
}

/// The soak's body breathes the home's air the bridges publish, from the first step: the survival
/// context is the sealed home's (oxygenated, the home's own temperature and humidity), not the
/// boot's default and not vacuum.
///
/// Seen red 2026-11-15 on its first run, on a real defect, BUG-317: "the sealed home's air reads
/// unbreathable: EnvironmentContext { sealed: true, oxygenated: false, ambient_temp_c: 19.850006,
/// ... }". The home's air status started all zeros until the first atmosphere step, so the first
/// step breathed vacuum: a frame in the windowed game, a whole 600 s step in the soak, where the
/// routine's person suffocated in it ("the person died following the routine: Some((600.0,
/// \"suffocation\"))") and the day, split-step and ladder tests all moved by the death. The world
/// entry publishes the home's air now (atmosphere::publish_air_status). With
/// `survival_env::publish` taken out of `bridges_slice` it fails "the bridges published no
/// environment around the body" (no `environment_context` slot, which increment 2's soak ran on).
#[test]
fn the_soaks_body_breathes_the_homes_air() {
    let mut home = sim_home::enter(&data_dir(), scratch_saves("env"));
    // The home's own air (its enclosed space) as the first step begins: the bridges publish the
    // air around the body from it before that step's tick.
    let (home_c, home_rh) = {
        use humanity_systems::atmosphere::{EnclosedSpace, HomeAir};
        let mut q = home.sim.game_world.world.query::<(&HomeAir, &EnclosedSpace)>();
        let (_, (_, s)) = q.iter().next().expect("the homestead has its own air space");
        (s.atmosphere.temperature_k - 273.15, s.atmosphere.humidity.clamp(0.0, 1.0))
    };
    home.step(600.0);
    let env = home
        .sim
        .data_store
        .get::<humanity_core::ecs::components::EnvironmentContext>("environment_context")
        .cloned()
        .expect("the bridges published no environment around the body");
    assert!(env.oxygenated, "the sealed home's air reads unbreathable: {env:?}");
    // The air around the body is the home's own, where the person stands outside a grow room.
    let obs = home.read();
    let (air_c, rh) = (get(&obs, "env.air_c"), get(&obs, "env.humidity"));
    assert!(
        (air_c - f64::from(home_c)).abs() < 1e-3 && (rh - f64::from(home_rh)).abs() < 1e-4,
        "the air around the body ({air_c} C at {rh}) is not the home's ({home_c} C at {home_rh})"
    );
    // And the body breathed it: ten minutes in, its blood oxygen is full.
    assert!(get(&obs, "vitals.oxygen") > 99.0, "blood oxygen fell to {} in the first ten minutes at home", get(&obs, "vitals.oxygen"));
}

/// The routine's acts fall due at their hours of the home's day, once each, across midnight.
#[test]
fn a_routines_acts_fall_due_once_at_their_hours() {
    use sim_routine::{due_between, Act, ActKind};
    let act = |hour: f64, act: ActKind| Act { hour, act, what: String::new(), day: None };
    let acts = vec![act(7.0, ActKind::Drink), act(22.5, ActKind::Sleep)];
    let due = |a: f64, b: f64| due_between(&acts, 24.0, a, b).into_iter().map(|(_, i)| acts[i].act).collect::<Vec<_>>();
    assert_eq!(due(6.0, 7.0), vec![ActKind::Drink], "an act at the end of the span is due");
    assert!(due(7.0, 8.0).is_empty(), "an act at the start of the span was due last time");
    assert_eq!(due(22.0, 31.0), vec![ActKind::Sleep, ActKind::Drink], "across midnight, in order");
    assert_eq!(due(0.0, 72.0).len(), 6, "three days, two acts a day");
}

/// A person following the daily routine through three game days at the safe step: the routine's
/// requests reach the systems through the frame's bridges, the person drinks from the kit's jugs
/// and then from bottles filled at the home's tank, eats the kit's rations, sleeps at night and is
/// alive at the end, where the untouched home's person is dead of thirst 2.6 days in.
///
/// Seen red 2026-11-15 with the requests not carried (`frame_sim_requests::carry` taken out of
/// `bridges_slice`): "the person died following the routine: Some((223800.0, \"dehydration\"))",
/// drinks 0 and meals 0, every act a miss ("drink request did nothing (water_purified_0)": 18,
/// "eat request did nothing (ration_basic_0)": 9). Before BUG-317's fix it died of suffocation at
/// 600 s, the first step.
#[test]
fn a_person_following_the_routine_drinks_eats_and_lives() {
    let tol = tolerances();
    let span = sim_soak::run_span(&data_dir(), tol.seed, tol.safe_step_s, 3.0 * DAY_S, None, Some(&daily()));
    let log = span.routine.clone().expect("the run kept no routine log");
    assert!(span.death.is_none(), "the person died following the routine: {:?}; {log:?}", span.death);
    assert!(log.drinks >= 12, "{} drinks in three days: {log:?}", log.drinks);
    assert!(log.water_drawn_l > 0.9, "no water drawn from the tank once the jugs ran out ({} L): {log:?}", log.water_drawn_l);
    assert!(log.meals >= 3, "{} meals in three days: {log:?}", log.meals);
    assert!(log.sleeps >= 2, "{} nights in bed in three days: {log:?}", log.sleeps);
    assert!(get(&span.end, "vitals.hydration") > 30.0, "hydration fell to {} following the routine", get(&span.end, "vitals.hydration"));
    assert!(get(&span.end, "vitals.energy") > 10.0, "rest fell to {} with a night in bed each day", get(&span.end, "vitals.energy"));
}

/// The split-step test over the bridges' quantities: a span in which the person drinks from a
/// bottle filled at the tank and eats a ration, stepped at the windowed frame's dt and coarsely,
/// lands the backpack (`pack.items`, `pack.water_l`), the body's environment and every other
/// quantity within its tolerance.
///
/// Seen red 2026-11-15 with the requests not carried (`frame_sim_requests::carry` taken out of
/// `bridges_slice`): "the span's drink and meal at the frame's dt: RoutineLog { drinks: 0, meals:
/// 0, ... missed: {\"eat request did nothing (ration_basic_0)\": 1, \"nothing to drink after
/// fetching\": 1, \"the tank's Fill did nothing\": 1} }". Before BUG-317's fix its coarse steps
/// were off by the first step's vacuum: "vitals.hydration (FoodSystem) at 60 s off by 0.3228
/// (allowed 0.1419)" and "vitals.energy at 60 s off by 0.3125 (allowed 0.0539)".
#[test]
fn a_routines_span_stepped_at_the_frames_dt_and_coarsely_agrees() {
    let tol = tolerances();
    let (start_h, _) = {
        let home = sim_home::enter(&data_dir(), scratch_saves("routine-start"));
        sim_routine::local_hours(&home.sim.data_store)
    };
    let day_h = 24.0;
    let hour = |s: f64| (start_h + s / 3600.0).rem_euclid(day_h);
    let routine = sim_routine::Routine {
        id: "split".into(),
        name: "a drink from the tank and a ration".into(),
        foods: vec!["ration_basic_0".into()],
        // Bottles only, so the drink is filled at the tank first (the kit's jugs are left).
        drinks: vec!["water_bottle_0".into()],
        acts: vec![sim_routine::Act { hour: hour(5.0), act: sim_routine::ActKind::Drink, what: String::new(), day: None }, sim_routine::Act { hour: hour(10.0), act: sim_routine::ActKind::Eat, what: String::new(), day: None }],
    };
    // Long enough for both acts at the coarsest step: each takes two steps, one act starts a step.
    let span_s = 300.0;
    let frame_dt = 1.0 / 60.0;
    let reference = sim_soak::run_span(&data_dir(), tol.seed, frame_dt, span_s, None, Some(&routine));
    let log = reference.routine.clone().expect("a routine log");
    assert_eq!((log.drinks, log.meals), (1, 1), "the span's drink and meal at the frame's dt: {log:?}");
    assert!(log.water_drawn_l > 0.4, "the Fill drew no water at the frame's dt: {log:?}");
    let mut off = Vec::new();
    for step_s in [1.0, 10.0, 60.0] {
        let run = sim_soak::run_span(&data_dir(), tol.seed, step_s, span_s, None, Some(&routine));
        for v in sim_soak::judge(&tol, &reference.first, &reference.end, &run.end) {
            if !v.holds() {
                off.push(format!("{} ({}) at {step_s} s off by {} (allowed {}): {} against {}", v.id, v.system, v.error, v.allowed, v.coarse, v.reference));
            }
        }
    }
    assert!(off.is_empty(), "{span_s} game seconds with a drink and a meal at the frame's {frame_dt:.4} s step against coarse steps:\n  {}", off.join("\n  "));
}

/// THE SOAK SHOWS WHAT A ROUTINE PRACTISED, TRY BY TRY (skills rung 2, 2026-11-15): the attempts the
/// SkillSystem weighed (`PracticeMemory::tries`) reach the routine's log once each, in order, and the
/// report's practice lines show each task's tries with what practice quality made them worth, so a
/// soak whose person harvests shows the repeat curve. Measured from what the skills kept, never from
/// what was asked.
///
/// Seen red 2026-11-15 with `practice_since` returning nothing: "the soak read 0 of the 4 tries the
/// skills weighed".
#[test]
fn the_soak_shows_each_practice_try_and_its_value() {
    use humanity_systems::skills::quality::{LearningMode, PracticeWhy};
    use humanity_systems::skills::PlayerSkills;
    let why = |attempt: u32, variety: f32, first_time: f32, gained: u32| PracticeWhy {
        task: "harvest.vegetable_lettuce_0".into(), base: 10, gained, at: 0.0, mode: LearningMode::Realistic, attempt,
        full_attempts: 2, first_time, variety, guide: None, mentor: None, difficulty: 1.0, difficulty_gap: None, quality: 1.0, transfer: 1.0,
    };
    let mut world = hecs::World::new();
    let mut skills = PlayerSkills::new();
    skills.memory.note_try("farming", why(1, 1.0, 1.5, 15));
    skills.memory.note_try("farming", why(2, 1.0, 1.0, 10));
    let player = world.spawn((skills, humanity_core::ecs::components::Controllable));
    let (first, seen) = sim_routine::practice_since(&world, 0);
    world.get::<&mut PlayerSkills>(player).unwrap().memory.note_try("farming", why(3, 0.75, 1.0, 8));
    world.get::<&mut PlayerSkills>(player).unwrap().memory.note_try("farming", why(4, 0.5625, 1.0, 6));
    let (second, seen2) = sim_routine::practice_since(&world, seen);
    let all: Vec<_> = first.iter().chain(second.iter()).cloned().collect();
    assert_eq!(all.len(), 4, "the soak read {} of the 4 tries the skills weighed", all.len());
    assert_eq!((seen, seen2), (2, 4), "each try is read once");
    assert_eq!(all.iter().map(|t| t.attempt).collect::<Vec<_>>(), vec![1, 2, 3, 4], "in order");
    assert!((all[0].value - 1.5).abs() < 1e-6 && (all[3].value - 0.5625).abs() < 1e-6, "with their values: {all:?}");
    let text = sim_soak::practice_text(&all);
    assert!(text.contains("farming harvest.vegetable_lettuce_0: #1 x1.50 (15/10), #2 x1.00 (10/10), #3 x0.75 (8/10), #4 x0.56 (6/10)"), "the report shows the curve:\n{text}");
}

// -- BUG-333: the routine soak repeats, in one process and across two ------------------------------

/// Everything a run decides, one line per thing, exactly (an f64's Debug form round-trips), the
/// wall times left out: what two runs of one seed must share, line for line.
fn decided(span: &sim_soak::Span) -> Vec<String> {
    let mut out = vec![format!("start {:?}", span.start), format!("first {:?}", span.first)];
    out.extend(span.marks.iter().map(|(t, o)| format!("mark {t:?} {o:?}")));
    out.push(format!("end {:?}", span.end));
    out.push(format!("steps {}", span.steps));
    out.push(format!("death {:?}", span.death));
    out.extend(span.notices.iter().map(|n| format!("notice {n:?}")));
    out.push(format!("routine {:?}", span.routine));
    out
}

/// The daily routine's seven-day soak (`--sim soak --routine daily`): the safe step, the
/// tolerances file's seed, read at the end of each day.
fn routine_soak() -> Vec<String> {
    let tol = tolerances();
    decided(&sim_soak::run_span(&data_dir(), tol.seed, tol.safe_step_s, 7.0 * DAY_S, Some(DAY_S), Some(&daily())))
}

/// Where two runs' lines first part, in words (None when they match): the first differing line
/// from just before the point it parts, and the notices only one of the runs had.
fn first_parting(a: &[String], b: &[String]) -> Option<String> {
    let at = (0..a.len().max(b.len())).find(|&i| a.get(i) != b.get(i))?;
    let differ = (0..a.len().max(b.len())).filter(|&i| a.get(i) != b.get(i)).count();
    let (x, y) = (a.get(at).map_or("(no line)", String::as_str), b.get(at).map_or("(no line)", String::as_str));
    let same = x.chars().zip(y.chars()).take_while(|(p, q)| p == q).count();
    let from = same.saturating_sub(40);
    let clip = |s: &str| format!("{}{}", if from > 0 { "..." } else { "" }, s.chars().skip(from).take(160).collect::<String>());
    let only = |p: &[String], q: &[String]| p.iter().filter(|l| l.starts_with("notice ") && !q.contains(l)).take(4).cloned().collect::<Vec<_>>();
    Some(format!(
        "{differ} of {} lines differ ({} then {} lines); the first, line {}:\n  {}\nthen\n  {}\nnotices only the first run had: {:?}\nnotices only the second run had: {:?}",
        a.len().max(b.len()),
        a.len(),
        b.len(),
        at + 1,
        clip(x),
        clip(y),
        only(a, b),
        only(b, a)
    ))
}

/// The environment variable that turns `the_routine_soak_in_a_process_of_its_own` on: the file it
/// writes its run's lines to.
const REPEAT_CHILD: &str = "HUMANITY_SOAK_REPEAT_CHILD";

/// THE ROUTINE SOAK REPEATS (BUG-333, 2026-11-20): the daily routine's seven days, run twice in
/// this process and once in a process of its own, decide the same thing to the last bit: every
/// day's state, every notice, every act. The headless runner's promise is "same inputs, same
/// result"; a rig that judges a run that does not repeat judges luck. A second process starts its
/// thread generators and its hash maps' keys afresh, so a draw or an order that is fixed within a
/// process and not across processes is caught too.
///
/// Seen red 2026-11-20 before the fix (the illness a raw meal can bring was rolled on the thread's
/// unseeded generator, food.rs, so the home storage's rice poisoned some runs and not others), on
/// every run it was given: "two runs of the routine soak in one process part: 27 of 46 lines
/// differ (46 then 44 lines); the first, line 4: ... \"vitals.hydration\": 85.29580688476563 ...
/// then ... \"vitals.hydration\": 94.79166412353516 ...; notices only the first run had: [\"Something
/// you ate has made you ill: Food Poisoning. ...\", \"The Food Poisoning has passed. ...\"]" (line 4
/// is the end of day 2).
#[test]
fn the_routine_soak_repeats_in_one_process_and_across_two() {
    let a = routine_soak();
    let b = routine_soak();
    if let Some(why) = first_parting(&a, &b) {
        panic!("two runs of the routine soak in one process part: {why}");
    }
    // The same run in a process of its own: this test program, asked for the child test alone.
    let out = std::env::temp_dir().join(format!("hos-soak-repeat-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&out);
    let name = format!("{}::the_routine_soak_in_a_process_of_its_own", module_path!().split_once("::").map_or(module_path!(), |(_, rest)| rest));
    let status = std::process::Command::new(std::env::current_exe().expect("this test program's path"))
        .args([name.as_str(), "--exact", "--ignored", "--test-threads=1", "-q"])
        .env(REPEAT_CHILD, &out)
        .stdout(std::process::Stdio::null())
        .status()
        .expect("run this test program again");
    assert!(status.success(), "the routine soak in a process of its own ({name}) failed: {status}");
    let text = std::fs::read_to_string(&out).unwrap_or_else(|e| panic!("the child process wrote no run to {}: {e}", out.display()));
    let _ = std::fs::remove_file(&out);
    let c: Vec<String> = text.lines().map(str::to_string).collect();
    if let Some(why) = first_parting(&a, &c) {
        panic!("the routine soak in this process and in a process of its own part: {why}");
    }
}

/// The child half of `the_routine_soak_repeats_in_one_process_and_across_two`: one run of the
/// routine soak, its lines written to the file `REPEAT_CHILD` names. Ignored, and doing nothing
/// without that variable, so only the parent test runs it.
#[test]
#[ignore = "the child half of the_routine_soak_repeats_in_one_process_and_across_two, which runs it in a process of its own"]
fn the_routine_soak_in_a_process_of_its_own() {
    let Some(out) = std::env::var_os(REPEAT_CHILD) else { return };
    std::fs::write(&out, routine_soak().join("\n")).expect("write the child run's lines");
}

// -- The survival quests, played through in the soak (2026-11-20, docs/design/quest-chains.md) --

/// EVERY SURVIVAL QUEST FINISHES IN THE HEADLESS SIM RUNNER, ON THE REAL SYSTEMS. A person
/// following data/sim/routines/survival_week.ron (the daily routine's day, plus the clicks the
/// survival chains ask for: Accept, look at the vitals, open the guides and the how-to, mix the
/// oral rehydration salts, plant, compost; no nap) for 70 game days at the safe step finishes every quest
/// of data/quests/survival.ron: Water First's drinks and its day of water counted by the intake
/// ledger and its day of Hydration held, Food First's 2,000 kcal day, Keep It, Grow Your Own's
/// radish grown, harvested and eaten, A Night's Sleep's two days of Energy, Clean Hands' three
/// days of Waste held under 75, Ready for Accidents and When Water Leaves You's salts mixed and
/// drunk. Start Farming then starts through its join (`after: AnyOf`), its other quest, Build
/// First Habitat, never done. No quest event is made up here: every one comes from the system
/// that reports it.
///
/// AND A REGULAR SLEEPER IS NEVER FATIGUED (2026-11-21): up at 06:40, in bed at 22:30, no nap, the
/// lowest Energy from the first morning on is 25 or more (the run's first day starts at midnight,
/// 22.5 hours awake: `RoutineLog::lowest_energy`). Tiredness at bedtime is real; the fatigue
/// slowdown is for staying up late or hard work (food.rs ENERGY_DECAY_PER_SEC, 17 hours to the
/// line, Williamson and Feyer 2000).
/// AND COOK IT THROUGH (2026-11-21, cooking rung 1): the kit's eggs boiled on the stove at the
/// station's pick, a dish cooked through and safe, which its Cook step counts. Seen red with the
/// routine's three Craft cook_boiled_eggs acts taken out: "the soak finished [...8 quests] of the
/// survival quests; under way [\"gs_first_steps\", \"food_cook\", \"tutorial_farming_basics\"]".
///
/// Seen red 2026-11-21 with the nap gone and the drain back at 75 over 16 hours: "a regular
/// sleeper went under the fatigue line: Energy 24.2 at 22.67 h" (up at 06:40, the lowest at
/// 22:40). With the 17-hour drain the lowest is 28.7 at 22:40.
///
/// Seen red 2026-11-20 before BUG-337's fix (Energy drained while asleep): "the soak finished
/// [\"aid_first\", \"water_first\", \"food_first\", \"clean_first\", \"food_keep\", \"aid_rehydrate\"] of
/// the survival quests; under way [\"gs_first_steps\", \"rest_first\", ...]", Energy every 3 h
/// "88, 74, 62, 48, 34, 20, 5, 0" through each night; and, before Water First's jerrycan reward
/// moved, "the person died playing the survival quests: Some((2316599.97, \"starvation\"))" (the
/// 24 L reward filled the backpack, so food from storage no longer fitted).
#[test]
fn the_survival_chains_finish_in_the_soak() {
    let tol = tolerances();
    let routine = sim_routine::Routine::load(&data_dir(), "survival_week").expect("data/sim/routines/survival_week.ron");
    // 70 days: Grow Your Own's lettuce takes 45 days at full health (data/plants.csv), and a crop
    // grows at its health (farming::step_path::growth_factor): a 50-day run ended with it unripe.
    let span = sim_soak::run_span(&data_dir(), tol.seed, tol.safe_step_s, 70.0 * DAY_S, Some(3.0 * 3600.0), Some(&routine));
    let log = span.routine.clone().expect("the run kept no routine log");
    let (done, under_way) = &span.quests;
    assert!(span.death.is_none(), "the person died playing the survival quests: {:?}; {log:?}", span.death);
    let (low, at) = log.lowest_energy.expect("the routine read no Energy");
    assert!(
        low >= 25.0,
        "a regular sleeper went under the fatigue line: Energy {low:.1} at {at:.2} h; Energy every 3 h of the first days {:?}",
        span.marks.iter().take(16).map(|(_, o)| get(o, "vitals.energy").round()).collect::<Vec<_>>()
    );
    let survival: Vec<String> = humanity_systems::quests::QuestRegistry::from_ron_dir(&data_dir().join("quests"))
        .quests
        .values()
        .filter(|q| q.starts == ["homestead"])
        .map(|q| q.id.clone())
        .collect();
    assert!(survival.len() >= 8, "the survival quests load: {survival:?}");
    let finished: Vec<&String> = survival.iter().filter(|id| done.contains(id)).collect();
    assert_eq!(
        finished.len(),
        survival.len(),
        "the soak finished {finished:?} of the survival quests; under way {under_way:?}; crops {} (grown {:.1} days each on average, health {:.0}); Energy every 3 h of the first days {:?}; notices {:?}; asked {:?}; {log:?}",
        get(&span.end, "crops.count"),
        get(&span.end, "crops.growing_s") / get(&span.end, "crops.count").max(1.0) / DAY_S,
        get(&span.end, "crops.health") / get(&span.end, "crops.count").max(1.0),
        span.marks.iter().take(32).map(|(_, o)| get(o, "vitals.energy").round()).collect::<Vec<_>>(),
        span.notices.iter().filter(|n| n.contains("sleep") || n.contains("slept") || n.contains("lie down") || n.contains("Plant") || n.contains("plant") || n.starts_with("Quest complete")).take(16).collect::<Vec<_>>(),
        log.asked
    );
    assert!(
        under_way.iter().chain(done.iter()).any(|q| q == "tutorial_farming_basics"),
        "Start Farming started through its join after Grow Your Own; under way {under_way:?}"
    );
    assert!(!done.iter().any(|q| q == "tutorial_first_habitat"), "the join's other quest was never done");
}

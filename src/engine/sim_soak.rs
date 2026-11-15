//! `HumanityOS --sim soak` and `--sim ladder`: the homestead's systems stepped as fast as the CPU
//! allows, with no window, no GPU and no listener (the headless sim runner's increment 2,
//! 2026-11-15; docs/design/headless-sim-runner.md sections 3 and 5).
//!
//! - **soak**: a fresh homestead (sim_home.rs) stepped N game days at the safe step, the state of
//!   every system at the end of each day, the wall time and the days per second, and the step's
//!   measured error: the first `check_hours` of the same run stepped again at the reference step
//!   and every system compared within its tolerance. A step past the safe one is refused unless
//!   forced (the design's risk 6: never trust a coarse step more than it was measured).
//! - **ladder**: one span stepped at each step of the ladder, every system judged against the
//!   reference step: which systems hold at which step, and the largest step at which all of them
//!   hold (the home's safe step, recorded in data/sim/step_tolerances.ron).
//!
//! Every run starts from a fresh homestead on a thread of its own (the save layer keeps its live
//! save per thread), with saves in a scratch folder that is removed afterwards (never the
//! player's saves), and the run's seed (humanity_systems::sim_seed), so the same command repeats.
//! The time speed stays at its default, 1, so a second of step is a game second.
//!
//! No GPU, window, painter or `EngineState` in here (tests/sim_gpu_free_lint.rs).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::engine::sim_home::{self, OBSERVABLES};

/// The tolerances file, under the data folder.
pub(crate) const TOLERANCES_FILE: &str = "sim/step_tolerances.ron";
/// Game seconds in a day (an Earth day; the soak keeps the default calendar).
pub(crate) const DAY_S: f64 = humanity_systems::time::EARTH_DAY_S;

/// data/sim/step_tolerances.ron: how coarse a step each system's quantities stand, and the step
/// the ladder found safe.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct StepTolerances {
    /// The fine step every coarser one is judged against, game seconds.
    pub reference_step_s: f64,
    /// The largest step at which every quantity held on the last ladder: the soak's step unless
    /// told otherwise, and the most it takes without `--force`.
    pub safe_step_s: f64,
    /// The steps the ladder tries, game seconds, smallest first.
    pub ladder_s: Vec<f64>,
    /// The span the ladder and the soak's error check step, game hours.
    pub check_hours: f64,
    /// The step the soak's error check compares its step against: one the ladder holds to the
    /// reference step, fine enough to judge a coarse step and coarse enough to keep a soak a
    /// matter of seconds (a day at the reference step is over a minute of wall time).
    pub check_step_s: f64,
    /// The seed every run starts from.
    pub seed: u64,
    /// One row per quantity (sim_home::OBSERVABLES).
    pub observables: Vec<Tolerance>,
}

/// How far one quantity may stray at a coarse step: `abs` in its own unit, plus `rel` times how
/// far it moved over the span at the reference step.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Tolerance {
    pub id: String,
    pub abs: f64,
    pub rel: f64,
    /// Why this much, in words.
    #[serde(default)]
    pub why: String,
}

impl StepTolerances {
    /// Read from the game's data folder.
    pub(crate) fn load(data_dir: &Path) -> Result<Self, String> {
        let path = data_dir.join(TOLERANCES_FILE);
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        ron::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    fn row(&self, id: &str) -> Option<&Tolerance> {
        self.observables.iter().find(|t| t.id == id)
    }
}

/// Every quantity by name (sim_home::OBSERVABLES), at one moment.
pub(crate) type Obs = BTreeMap<&'static str, f64>;

/// One run from a fresh homestead.
#[derive(Debug, Clone)]
pub(crate) struct Span {
    /// Before the first step.
    pub start: Obs,
    /// Right after the first step, as it stands (`Homestead::read`): the baseline a quantity's
    /// movement is measured from, past what the first step only sets up (the home's air reads 0
    /// until the air step's first run).
    pub first: Obs,
    /// At each mark asked for (every `mark_s`), then at the end.
    pub marks: Vec<(f64, Obs)>,
    pub end: Obs,
    pub steps: u64,
    /// Wall seconds stepping (the entry and the readings not counted).
    pub wall_s: f64,
    /// Wall seconds entering the homestead.
    pub enter_s: f64,
    /// What the systems said, in order.
    pub notices: Vec<String>,
    /// When the player died, game seconds into the run, and of what (the death screen's cause).
    pub death: Option<(f64, String)>,
}

/// Step a fresh homestead `span_s` game seconds at `step_s`, reading every quantity every
/// `mark_s` (a step is shortened to land on a mark, so the marks are exact), on a thread of its
/// own with its own scratch saves.
pub(crate) fn run_span(data_dir: &Path, seed: u64, step_s: f64, span_s: f64, mark_s: Option<f64>) -> Span {
    let data_dir = data_dir.to_path_buf();
    std::thread::Builder::new()
        .name("sim-span".into())
        .stack_size(16 << 20)
        .spawn(move || span_on_this_thread(&data_dir, seed, step_s, span_s, mark_s))
        .expect("a thread for the run")
        .join()
        .unwrap_or_else(|p| std::panic::resume_unwind(p))
}

fn span_on_this_thread(data_dir: &Path, seed: u64, step_s: f64, span_s: f64, mark_s: Option<f64>) -> Span {
    let saves = scratch_saves();
    let t_enter = std::time::Instant::now();
    let mut home = sim_home::enter(data_dir, saves.clone());
    home.sim.data_store.insert(humanity_systems::sim_seed::SLOT, seed);
    // The garden planted, as the first hour asks (the opening quest's Plant step), so a run
    // grows crops. Planted by a step of no time, so it is in the ground at the start of every
    // run whatever the step: planted by the first real step, a crop grew from the end of that
    // step, one step's growth short per crop at a coarse step (seen in the ladder: 36 s at 10 s
    // for the tower's 4 crops), which measured the script, not the farm.
    home.plant_first_tower(data_dir);
    home.step(0.0);
    let enter_s = t_enter.elapsed().as_secs_f64();
    let start = home.observe();
    let mut marks = Vec::new();
    let mut t = 0.0_f64;
    let mut steps = 0_u64;
    let mut wall = 0.0_f64;
    let mut next_mark = mark_s.unwrap_or(f64::INFINITY);
    let mut first = None;
    let mut death = None;
    while t < span_s - 1e-9 {
        let t0 = std::time::Instant::now();
        // Step to the next mark (or the end) at `step_s`, the last step shortened to land on it.
        let until = next_mark.min(span_s);
        while t < until - 1e-9 {
            let dt = step_s.min(until - t);
            home.step(dt as f32);
            t += dt;
            steps += 1;
            if first.is_none() {
                first = Some(home.read());
            }
            if death.is_none() {
                death = home.gui.player_death_cause.clone().map(|cause| (t, cause));
            }
        }
        wall += t0.elapsed().as_secs_f64();
        if t >= next_mark - 1e-9 && t < span_s - 1e-9 {
            marks.push((t, home.observe()));
            next_mark += mark_s.unwrap_or(f64::INFINITY);
        }
    }
    let end = home.observe();
    let notices = std::mem::take(&mut home.notices);
    drop(home);
    let _ = std::fs::remove_dir_all(&saves);
    let first = first.unwrap_or_else(|| start.clone());
    Span { start, first, marks, end, steps, wall_s: wall, enter_s, notices, death }
}

/// A saves folder of the run's own in the system's temporary folder, empty: no player's save is
/// read or written.
fn scratch_saves() -> PathBuf {
    static RUN: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("humanityos-sim-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// One quantity at a coarse step against the reference.
#[derive(Debug, Clone)]
pub(crate) struct Verdict {
    pub id: &'static str,
    pub system: &'static str,
    pub reference: f64,
    pub coarse: f64,
    pub error: f64,
    pub allowed: f64,
    /// Why that much is allowed (the tolerance row's words).
    pub why: String,
}

impl Verdict {
    pub(crate) fn holds(&self) -> bool {
        self.error <= self.allowed
    }
}

/// Every quantity at `coarse` against `reference`, both run from one start, within its
/// tolerance; `baseline` is the reference run's reading after its first step, which a quantity's
/// movement is measured from (`Span::first`). A quantity with no tolerance row is allowed no error
/// at all (a new observable gets its row, tolerances_cover_every_observable).
pub(crate) fn judge(tol: &StepTolerances, baseline: &Obs, reference: &Obs, coarse: &Obs) -> Vec<Verdict> {
    OBSERVABLES
        .iter()
        .filter_map(|o| {
            let r = *reference.get(o.id)?;
            let c = coarse.get(o.id).copied().unwrap_or(f64::NAN);
            let moved = (r - baseline.get(o.id).copied().unwrap_or(r)).abs();
            let row = tol.row(o.id);
            let allowed = row.map_or(0.0, |t| t.abs + t.rel * moved);
            let why = row.map_or_else(|| "no tolerance row: no error allowed".to_string(), |t| t.why.clone());
            let error = if c.is_nan() { f64::INFINITY } else { (c - r).abs() };
            Some(Verdict { id: o.id, system: o.system, reference: r, coarse: c, error, allowed, why })
        })
        .collect()
}

/// One rung of the ladder: a step and how every quantity fared at it.
#[derive(Debug, Clone)]
pub(crate) struct Rung {
    pub step_s: f64,
    pub verdicts: Vec<Verdict>,
    pub wall_s: f64,
    pub steps: u64,
}

impl Rung {
    pub(crate) fn holds(&self) -> bool {
        self.verdicts.iter().all(Verdict::holds)
    }
}

/// The ladder: `span_s` at the reference step, then at each of `steps`, every quantity judged.
pub(crate) fn ladder(data_dir: &Path, tol: &StepTolerances, span_s: f64, steps: &[f64]) -> (Span, Vec<Rung>) {
    let reference = run_span(data_dir, tol.seed, tol.reference_step_s, span_s, None);
    let rungs = steps
        .iter()
        .map(|&step_s| {
            let run = run_span(data_dir, tol.seed, step_s, span_s, None);
            let verdicts = judge(tol, &reference.first, &reference.end, &run.end);
            Rung { step_s, verdicts, wall_s: run.wall_s, steps: run.steps }
        })
        .collect();
    (reference, rungs)
}

/// The largest step at which every quantity holds, every smaller rung holding too.
pub(crate) fn safe_step(rungs: &[Rung]) -> Option<f64> {
    rungs.iter().take_while(|r| r.holds()).last().map(|r| r.step_s)
}

/// The ladder as words: per quantity, the largest step it holds at, and each rung's verdicts.
pub(crate) fn ladder_text(tol: &StepTolerances, span_s: f64, reference: &Span, rungs: &[Rung]) -> String {
    let mut out = format!(
        "Step ladder over {:.1} game hours, judged against {} s steps ({} steps, {:.2} s of wall time).\n",
        span_s / 3600.0,
        tol.reference_step_s,
        reference.steps,
        reference.wall_s
    );
    for r in rungs {
        out += &format!(
            "  {:>6} s: {} ({} steps, {:.3} s)\n",
            r.step_s,
            if r.holds() { "every system holds".to_string() } else { format!("{} off", r.verdicts.iter().filter(|v| !v.holds()).count()) },
            r.steps,
            r.wall_s
        );
    }
    out += "Which systems hold at which step (the largest step each quantity stays within its tolerance at):\n";
    for o in OBSERVABLES {
        let mut best: Option<f64> = None;
        for r in rungs {
            match r.verdicts.iter().find(|v| v.id == o.id) {
                Some(v) if v.holds() => best = Some(r.step_s),
                Some(_) => break,
                None => {}
            }
        }
        let worst = rungs
            .iter()
            .filter_map(|r| r.verdicts.iter().find(|v| v.id == o.id && !v.holds()).map(|v| (r.step_s, v)))
            .next();
        out += &format!(
            "  {:<20} {:<18} holds to {:>8}{}\n",
            o.id,
            o.system,
            best.map_or("none".to_string(), |s| format!("{s} s")),
            worst.map_or(String::new(), |(s, v)| format!("; at {s} s off by {:.4} (allowed {:.4})", v.error, v.allowed))
        );
    }
    out += &format!(
        "Safe step: {} (recorded: {} s).\n",
        safe_step(rungs).map_or("none of the rungs".to_string(), |s| format!("{s} s")),
        tol.safe_step_s
    );
    out
}

/// What `--sim soak` was asked for.
#[derive(Debug, Clone)]
pub(crate) struct SoakAsk {
    pub days: f64,
    /// None: the safe step.
    pub step_s: Option<f64>,
    pub force: bool,
    /// None: the tolerances file's.
    pub check_hours: Option<f64>,
    pub seed: Option<u64>,
}

/// A finished soak.
#[derive(Debug, Clone)]
pub(crate) struct SoakReport {
    pub days: f64,
    pub step_s: f64,
    pub safe_step_s: f64,
    pub check_hours: f64,
    pub seed: u64,
    pub run: Span,
    /// The first `check_hours` at the soak's step, judged against the reference step.
    pub check: Vec<Verdict>,
    /// The step the check was judged against (the tolerances file's check step, or its reference
    /// step for a soak as fine as that).
    pub reference_step_s: f64,
    pub check_wall_s: f64,
}

/// Run a soak. Refuses (Err) a step past the safe one unless forced.
pub(crate) fn soak(data_dir: &Path, ask: &SoakAsk) -> Result<SoakReport, String> {
    let tol = StepTolerances::load(data_dir)?;
    let step_s = ask.step_s.unwrap_or(tol.safe_step_s);
    if !(step_s > 0.0) || !(ask.days > 0.0) {
        return Err(format!("a soak needs a positive step and a positive number of days (asked: step {step_s} s, {} days)", ask.days));
    }
    if step_s > tol.safe_step_s && !ask.force {
        return Err(format!(
            "a {step_s} s step is coarser than the safe step, {} s, the largest at which every system held on the last ladder \
             (data/{TOLERANCES_FILE}). Run `--sim ladder` to measure again, or add --force to soak at it anyway; \
             the report will show how far off it is.",
            tol.safe_step_s
        ));
    }
    let check_hours = ask.check_hours.unwrap_or(tol.check_hours);
    let check_s = (check_hours * 3600.0).min(ask.days * DAY_S);
    let seed = ask.seed.unwrap_or(tol.seed);
    // The soak itself, read at the end of every day and at the end of the error check's span.
    let run = run_span(data_dir, seed, step_s, ask.days * DAY_S, Some(DAY_S.min(check_s)));
    // The error check: the same start stepped at the check's step (the reference step for a soak
    // as fine as the check's) over the check's span.
    let check_step_s = if step_s > tol.check_step_s { tol.check_step_s } else { tol.reference_step_s };
    let reference = run_span(data_dir, seed, check_step_s, check_s, None);
    let at_check = run
        .marks
        .iter()
        .find(|(t, _)| (*t - check_s).abs() < 1e-6)
        .map(|(_, o)| o)
        .unwrap_or(&run.end);
    let check = judge(&tol, &reference.first, &reference.end, at_check);
    Ok(SoakReport {
        days: ask.days,
        step_s,
        safe_step_s: tol.safe_step_s,
        check_hours: check_s / 3600.0,
        seed,
        run,
        check,
        reference_step_s: check_step_s,
        check_wall_s: reference.wall_s,
    })
}

impl SoakReport {
    /// Game days stepped per wall second.
    pub(crate) fn days_per_s(&self) -> f64 {
        self.days / self.run.wall_s.max(1e-9)
    }

    /// The report in words.
    pub(crate) fn text(&self) -> String {
        let r = &self.run;
        let mut out = format!(
            "Soak: {} game days of the homestead at a {} s step (safe step {} s), seed {}.\n\
             Stepped {} times in {:.2} s of wall time: {:.1} game days a second, {:.0}x real time \
             (entering the homestead took {:.2} s).\n",
            self.days,
            self.step_s,
            self.safe_step_s,
            self.seed,
            r.steps,
            r.wall_s,
            self.days_per_s(),
            self.days * DAY_S / r.wall_s.max(1e-9),
            r.enter_s
        );
        let off: Vec<&Verdict> = self.check.iter().filter(|v| !v.holds()).collect();
        out += &format!(
            "Measured error of the step: the first {:.1} game hours again at {} s steps ({:.2} s): {}.\n",
            self.check_hours,
            self.reference_step_s,
            self.check_wall_s,
            if off.is_empty() { "every system within its tolerance".to_string() } else { format!("{} quantities outside their tolerance", off.len()) }
        );
        for v in &self.check {
            out += &format!(
                "  {:<20} {:<18} off by {:.6} (allowed {:.6}){}\n",
                v.id,
                v.system,
                v.error,
                v.allowed,
                if v.holds() { String::new() } else { format!("  OUTSIDE: {}", v.why) }
            );
        }
        out += "Each system's state, at the start and at the end of each game day:\n";
        let mut days: Vec<(String, &Obs)> = vec![("start".into(), &r.start)];
        days.extend(r.marks.iter().filter(|(t, _)| (t / DAY_S).fract().abs() < 1e-9).map(|(t, o)| (format!("day {}", (t / DAY_S).round()), o)));
        days.push(("end".into(), &r.end));
        out += &format!("  {:<20}", "");
        for (name, _) in &days {
            out += &format!(" {name:>12}");
        }
        out += "\n";
        for o in OBSERVABLES {
            out += &format!("  {:<20}", o.id);
            for (_, obs) in &days {
                out += &match obs.get(o.id) {
                    Some(v) => format!(" {:>12}", short(*v)),
                    None => format!(" {:>12}", "-"),
                };
            }
            out += "\n";
        }
        out += "What each quantity is:\n";
        for o in OBSERVABLES {
            out += &format!("  {:<20} {} ({})\n", o.id, o.what, o.system);
        }
        match &r.death {
            Some((t, cause)) => out += &format!("The player died {:.1} game days in, of {cause}.\n", t / DAY_S),
            None => out += "The player lived through the run.\n",
        }
        out += &format!("The systems said {} things", r.notices.len());
        if let Some(last) = r.notices.last() {
            out += &format!("; the last: \"{last}\"");
        }
        out += ".\n";
        out
    }

    /// The report as JSON (what `--report` writes and the Dev page reads).
    pub(crate) fn json(&self) -> serde_json::Value {
        let obs = |o: &Obs| serde_json::json!(o);
        serde_json::json!({
            "days": self.days,
            "step_s": self.step_s,
            "safe_step_s": self.safe_step_s,
            "seed": self.seed,
            "steps": self.run.steps,
            "wall_s": self.run.wall_s,
            "enter_s": self.run.enter_s,
            "days_per_s": self.days_per_s(),
            "check": {
                "hours": self.check_hours,
                "reference_step_s": self.reference_step_s,
                "wall_s": self.check_wall_s,
                "holds": self.check.iter().all(Verdict::holds),
                "quantities": self.check.iter().map(|v| serde_json::json!({
                    "id": v.id, "system": v.system, "reference": v.reference, "at_step": v.coarse,
                    "error": v.error, "allowed": v.allowed, "holds": v.holds(),
                })).collect::<Vec<_>>(),
            },
            "start": obs(&self.run.start),
            "days_end": self.run.marks.iter().filter(|(t, _)| (t / DAY_S).fract().abs() < 1e-9).map(|(t, o)| serde_json::json!({ "day": (t / DAY_S).round(), "state": obs(o) })).collect::<Vec<_>>(),
            "end": obs(&self.run.end),
            "death": self.run.death.as_ref().map(|(t, cause)| serde_json::json!({ "day": t / DAY_S, "cause": cause })),
            "notices": self.run.notices.len(),
            "last_notice": self.run.notices.last(),
        })
    }
}

/// A number short enough for a table column.
fn short(v: f64) -> String {
    if v == 0.0 {
        "0".into()
    } else if v.abs() >= 1e6 {
        format!("{v:.4e}")
    } else if v.abs() >= 100.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.4}")
    }
}

/// The value after `flag` on the command line, parsed.
fn arg<T: std::str::FromStr>(args: &[String], flag: &str) -> Result<Option<T>, String> {
    match args.iter().position(|a| a == flag) {
        None => Ok(None),
        Some(i) => args
            .get(i + 1)
            .and_then(|v| v.parse().ok())
            .map(Some)
            .ok_or_else(|| format!("{flag} needs a number after it")),
    }
}

/// The usage words.
const USAGE: &str = "usage: HumanityOS --sim soak [--days N] [--step SECONDS] [--force] [--check-hours H] [--seed N] [--report FILE]\n       \
                     HumanityOS --sim ladder [--hours H] [--report FILE]";

/// `HumanityOS --sim ...` (src/main.rs dispatches here before any window, relay or update check):
/// the report on standard output and, with `--report`, as JSON in a file. Returns the exit code:
/// 0 done, 2 refused or asked wrongly.
pub fn cli_main(args: &[String]) -> i32 {
    match cli(args) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("{USAGE}");
            2
        }
    }
}

fn cli(args: &[String]) -> Result<(), String> {
    let i = args.iter().position(|a| a == "--sim").ok_or("no --sim")?;
    let what = args.get(i + 1).map(String::as_str).unwrap_or("");
    let data_dir = crate::native_app::find_data_dir();
    let report_path: Option<PathBuf> = args.iter().position(|a| a == "--report").and_then(|j| args.get(j + 1)).map(PathBuf::from);
    let (text, json) = match what {
        "soak" => {
            let ask = SoakAsk {
                days: arg(args, "--days")?.unwrap_or(7.0),
                step_s: arg(args, "--step")?,
                force: args.iter().any(|a| a == "--force"),
                check_hours: arg(args, "--check-hours")?,
                seed: arg(args, "--seed")?,
            };
            let report = soak(&data_dir, &ask)?;
            (report.text(), report.json())
        }
        "ladder" => {
            let tol = StepTolerances::load(&data_dir)?;
            let hours: f64 = arg(args, "--hours")?.unwrap_or(tol.check_hours);
            let (reference, rungs) = ladder(&data_dir, &tol, hours * 3600.0, &tol.ladder_s);
            let json = serde_json::json!({
                "hours": hours,
                "reference_step_s": tol.reference_step_s,
                "safe_step_s": safe_step(&rungs),
                "rungs": rungs.iter().map(|r| serde_json::json!({
                    "step_s": r.step_s, "holds": r.holds(), "steps": r.steps, "wall_s": r.wall_s,
                    "off": r.verdicts.iter().filter(|v| !v.holds()).map(|v| serde_json::json!({ "id": v.id, "system": v.system, "error": v.error, "allowed": v.allowed })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            });
            (ladder_text(&tol, hours * 3600.0, &reference, &rungs), json)
        }
        other => return Err(format!("--sim {other:?}: not a simulation run this build knows (soak, ladder)")),
    };
    println!("{text}");
    if let Some(path) = report_path {
        let body = serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?;
        std::fs::write(&path, body).map_err(|e| format!("{}: {e}", path.display()))?;
        println!("Report written to {}", path.display());
    }
    Ok(())
}

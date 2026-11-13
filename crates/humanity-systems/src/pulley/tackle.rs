//! The block and tackle's physics and the configurator that designs one (docs/design/pulleys.md,
//! "The physics" and "The configurator").
//!
//! THE MODEL, in one paragraph. A stage is one rope reeved between a fixed block and a moving
//! block; its ideal mechanical advantage is the number of parts of rope at the moving block. The
//! hauling part leaves the fixed block, so a stage of n parts runs over n sheaves (Crosby's
//! reeving, finding F6). Raising, each sheave passes on only its bearing's efficiency e of the
//! pull, so the parts carry P e, P e^2, ... P e^n and the real advantage is the sum of e^k for
//! k = 1..n: below n for every e < 1, and never above e / (1 - e) however many parts you add
//! (24 for bronze bushings, 49 for roller bearings). Stages stacked block on block multiply. The
//! rope taken in is the IDEAL advantage times the lift, whatever the friction: friction costs
//! force, never rope. In the Realistic mode the moving block's own mass and half the weight of the
//! rope hanging in the falls are lifted too, and a rope pulled past its working load limit is
//! refused with the numbers; the Simplified mode is the ideal machine, no friction, no failures.

use super::data::{Bearing, Material, Puller, PulleyData, Rope};
use super::geometry::BlockDims;

/// The two modes every deep system ships (CLAUDE.md, "Dual modes").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Friction per sheave, the gear's own weight, working load limits that refuse a lift.
    #[default]
    Realistic,
    /// The ideal machine: advantage = parts, nothing fails.
    Simplified,
}

/// What the configurator is asked.
#[derive(Debug, Clone, Copy)]
pub struct Inputs<'a> {
    pub load_kg: f64,
    pub lift_m: f64,
    /// Surface gravity where the lift happens, m/s^2 (Earth 9.81; each world's is its data).
    pub gravity: f64,
    pub puller: &'a Puller,
    pub rope: &'a Rope,
    pub bearing: &'a Bearing,
    pub mode: Mode,
    /// A rope size the person fixed, mm; None lets the configurator size it.
    pub rope_d_mm: Option<f64>,
}

/// One stage as designed.
#[derive(Debug, Clone, PartialEq)]
pub struct StageDesign {
    /// Parts of rope at the moving block (the stage's ideal advantage).
    pub parts: u32,
    pub fixed_sheaves: u32,
    pub moving_sheaves: u32,
    pub ideal_ma: f64,
    /// Sum of e^k over the parts (= parts in the Simplified mode).
    pub real_ma: f64,
    pub rope_d_mm: f64,
    /// The rope's working load limit at that size, newtons.
    pub rope_wll_n: f64,
    /// How far the moving block travels for the whole lift, metres.
    pub travel_m: f64,
    /// The blocks' pin-to-pin distance at the start of the lift, metres.
    pub span_m: f64,
    /// Rope reeved in this stage, metres.
    pub rope_m: f64,
    /// What the moving block lifts (the stage below's pull, or the load), newtons.
    pub load_n: f64,
    /// Tension in the hauling part: the highest in this stage's rope while raising, newtons.
    pub hauling_n: f64,
    /// The fixed block's load: the load plus the hauling part, newtons.
    pub anchor_n: f64,
    pub upper: BlockDims,
    pub lower: BlockDims,
}

/// What the configurator says overall about the system it chose.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Verdict {
    /// The puller can lift it straight; one block only turns the rope.
    Direct,
    /// A tackle does it.
    Tackle,
    /// No tackle within the data's limits does it (the reason says how far short).
    BeyondTackle,
}

/// Advice beside the result: what else would do the job better, said plainly.
#[derive(Debug, Clone, PartialEq)]
pub enum Advice {
    /// A geared chain hoist: its efficiency against this tackle's (F11).
    GearedHoist { hoist_key: String, hoist_efficiency: f64, tackle_efficiency: f64 },
    /// The lift takes long enough that more hands or a winch is the better answer.
    MoreHands { minutes: f64 },
    /// A winch's rating holds only on the drum's first layer (F8).
    FirstLayer,
    /// The hauling end must be reset this many times (its stroke is shorter than the rope taken in).
    Strokes { strokes: u64 },
}

/// Why a lift is refused in the Realistic mode, with the numbers.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// A stage's rope would be pulled past its working load limit.
    RopeOverLimit { stage: usize, rope_d_mm: f64, wll_n: f64, tension_n: f64 },
    /// No tackle within the limits gives enough advantage.
    NotEnoughAdvantage { best_real_ma: f64, needed_ma: f64 },
}

/// The configurator's answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// Stage 0 holds the load; the last stage's hauling part is the puller's.
    pub stages: Vec<StageDesign>,
    pub ideal_ma: f64,
    /// Load weight over pull: the friction and the gear's own weight included.
    pub real_ma: f64,
    /// real / ideal.
    pub efficiency: f64,
    /// The pull the puller must keep up, newtons.
    pub pull_n: f64,
    /// Rope taken in for the whole lift: ideal advantage x lift, metres.
    pub rope_pulled_m: f64,
    pub time_s: f64,
    pub total_rope_m: f64,
    pub rope_mass_kg: f64,
    pub blocks_mass_kg: f64,
    pub verdict: Verdict,
    pub advice: Vec<Advice>,
    pub refusal: Option<Refusal>,
}

/// One stage's real advantage: the sum of e^k for k = 1..parts.
pub fn stage_real_ma(parts: u32, efficiency: f64) -> f64 {
    let mut sum = 0.0;
    let mut p = 1.0;
    for _ in 0..parts {
        p *= efficiency;
        sum += p;
    }
    sum
}

/// Work out a system of the given parts per stage (stage 0 holds the load).
pub fn evaluate(inp: &Inputs, data: &PulleyData, mat: Material, parts: &[u32]) -> Outcome {
    let realistic = inp.mode == Mode::Realistic;
    let e = if realistic { inp.bearing.efficiency } else { 1.0 };
    let g = inp.gravity.max(0.0);
    let rules = &data.block;
    let weight = inp.load_kg.max(0.0) * g;
    let mut stages: Vec<StageDesign> = Vec::with_capacity(parts.len());
    let mut refusal = None;
    let mut below = weight;
    let mut travel = inp.lift_m.max(0.0);
    for (i, &n) in parts.iter().enumerate() {
        let n = n.max(1);
        let real_ma = stage_real_ma(n, e);
        let last = i + 1 == parts.len();
        let fixed_sheaves = n.div_ceil(2);
        let moving_sheaves = n / 2;
        // The gear's own weight is lifted too (Realistic): the moving block and half of each part
        // hanging between the blocks. Its size depends on the load it carries, so settle it in a
        // few rounds (it converges at once: the gear is a small share of the load).
        let mut load = below;
        let mut stage = None;
        for _ in 0..3 {
            let hauling = load / real_ma.max(1e-9);
            let d = inp.rope_d_mm.or_else(|| inp.rope.size_for(hauling)).unwrap_or(*inp.rope.diameters_mm.last().unwrap_or(&10.0));
            let lower = BlockDims::design(moving_sheaves, inp.rope, d, load, mat, rules);
            let upper = BlockDims::design(fixed_sheaves, inp.rope, d, load + hauling, mat, rules);
            let span = travel + rules.clearance_m + 0.5 * (upper.cheek_h + lower.cheek_h);
            let rope_m = n as f64 * span + if last { rules.lead_m } else { 1.0 };
            let falls_kg = n as f64 * span * inp.rope.kg_per_m(d);
            let next_load = if realistic { below + (lower.mass_kg + 0.5 * falls_kg) * g } else { below };
            stage = Some(StageDesign {
                parts: n,
                fixed_sheaves,
                moving_sheaves,
                ideal_ma: n as f64,
                real_ma,
                rope_d_mm: d,
                rope_wll_n: inp.rope.wll_n(d),
                travel_m: travel,
                span_m: span,
                rope_m,
                load_n: load,
                hauling_n: hauling,
                anchor_n: load + hauling,
                upper,
                lower,
            });
            if (next_load - load).abs() <= 1e-6 * load.max(1.0) {
                break;
            }
            load = next_load;
        }
        let s = stage.expect("a stage is designed in the first round");
        if realistic && refusal.is_none() && s.hauling_n > s.rope_wll_n {
            refusal = Some(Refusal::RopeOverLimit { stage: i, rope_d_mm: s.rope_d_mm, wll_n: s.rope_wll_n, tension_n: s.hauling_n });
        }
        below = s.hauling_n;
        travel *= n as f64;
        stages.push(s);
    }
    let ideal_ma: f64 = stages.iter().map(|s| s.ideal_ma).product();
    let pull_n = below;
    let real_ma = if pull_n > 0.0 { weight / pull_n } else { ideal_ma };
    let rope_pulled_m = inp.lift_m.max(0.0) * ideal_ma;
    let total_rope_m = stages.iter().map(|s| s.rope_m).sum();
    let rope_mass_kg = stages.iter().map(|s| s.rope_m * inp.rope.kg_per_m(s.rope_d_mm)).sum();
    let blocks_mass_kg = stages.iter().map(|s| s.upper.mass_kg + s.lower.mass_kg).sum();
    let verdict = if parts == [1] { Verdict::Direct } else { Verdict::Tackle };
    let mut out = Outcome {
        stages,
        ideal_ma,
        real_ma,
        efficiency: if ideal_ma > 0.0 { real_ma / ideal_ma } else { 1.0 },
        pull_n,
        rope_pulled_m,
        time_s: rope_pulled_m / inp.puller.speed_m_s.max(1e-9),
        total_rope_m,
        rope_mass_kg,
        blocks_mass_kg,
        verdict,
        advice: Vec::new(),
        refusal,
    };
    out.advice = advise(inp, data, &out);
    out
}

/// The advice for an outcome (the rules are in docs/design/pulleys.md, "When something else wins").
fn advise(inp: &Inputs, data: &PulleyData, out: &Outcome) -> Vec<Advice> {
    let mut v = Vec::new();
    // A person needing tackles stacked block on block is the job a geared chain hoist is for: name
    // the most efficient one in the data, and its efficiency beside this tackle's.
    if inp.puller.hand && out.stages.len() >= 2 {
        if let Some(h) = data.hoists.iter().max_by(|a, b| a.efficiency.total_cmp(&b.efficiency)) {
            v.push(Advice::GearedHoist { hoist_key: h.name_key.clone(), hoist_efficiency: h.efficiency, tackle_efficiency: out.efficiency });
        }
    }
    if inp.puller.hand && out.time_s > 600.0 {
        v.push(Advice::MoreHands { minutes: out.time_s / 60.0 });
    }
    if !inp.puller.hand && out.pull_n > 0.5 * inp.puller.force_n {
        v.push(Advice::FirstLayer);
    }
    let strokes = (out.rope_pulled_m / inp.puller.stroke_m.max(1e-9)).ceil() as u64;
    if strokes > 1 {
        v.push(Advice::Strokes { strokes });
    }
    v
}

/// Every parts-per-stage plan the search considers, best first: fewest stages (the simplest
/// rigging), then fewest sheaves, then the least rope taken in, then the shortest throw for the
/// last stage. A stage of one part in a compound system turns the rope and adds nothing, so a
/// compound stage has at least two parts.
fn plans(max_parts: u32, max_stages: u32) -> Vec<Vec<u32>> {
    let mut out: Vec<Vec<u32>> = (1..=max_parts).map(|n| vec![n]).collect();
    let mut level: Vec<Vec<u32>> = (2..=max_parts).map(|n| vec![n]).collect();
    for _ in 1..max_stages {
        let mut next = Vec::new();
        for p in &level {
            for n in 2..=max_parts {
                let mut q = p.clone();
                q.push(n);
                next.push(q);
            }
        }
        out.extend(next.iter().cloned());
        level = next;
    }
    let key = |p: &Vec<u32>| {
        let sheaves: u32 = p.iter().sum();
        let ideal: u64 = p.iter().map(|&n| n as u64).product();
        let throw: u64 = p[..p.len() - 1].iter().map(|&n| n as u64).product();
        (p.len(), sheaves, ideal, throw)
    };
    out.sort_by_key(key);
    out
}

/// The configurator: the best system for this load, lift, puller and rope (see `plans` for what
/// "best" means). Refuses with a reason in the Realistic mode when nothing within the data's
/// limits will do; in the Simplified mode the most advantage on offer is returned and nothing is
/// refused.
pub fn recommend(inp: &Inputs, data: &PulleyData, mat: Material) -> Outcome {
    let e = if inp.mode == Mode::Realistic { inp.bearing.efficiency } else { 1.0 };
    let weight = inp.load_kg.max(0.0) * inp.gravity.max(0.0);
    let force = inp.puller.force_n.max(1e-9);
    let all = plans(data.block.max_parts_per_stage.max(1), data.block.max_stages.max(1));
    let cheap = |p: &[u32]| p.iter().map(|&n| stage_real_ma(n, e)).product::<f64>();
    let mut first_refused: Option<Outcome> = None;
    for p in &all {
        // The friction alone first (cheap); the gear's weight and the rope's limit after.
        if weight / cheap(p) > force {
            continue;
        }
        let out = evaluate(inp, data, mat, p);
        if out.pull_n > force {
            continue;
        }
        if out.refusal.is_none() {
            return out;
        }
        first_refused.get_or_insert(out);
    }
    if let Some(out) = first_refused {
        return out;
    }
    // Nothing gives enough advantage: show the strongest plan, and say how far short it falls.
    let best = all.iter().max_by(|a, b| cheap(a).total_cmp(&cheap(b))).cloned().unwrap_or_else(|| vec![1]);
    let mut out = evaluate(inp, data, mat, &best);
    out.verdict = Verdict::BeyondTackle;
    if inp.mode == Mode::Realistic {
        out.refusal = Some(Refusal::NotEnoughAdvantage { best_real_ma: out.real_ma, needed_ma: weight / force });
    }
    out
}

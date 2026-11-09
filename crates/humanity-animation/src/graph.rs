//! The locomotion state machine with blend spaces (plan section 7's first column, rung 1, and the
//! simplified mode for good: section 8.6).
//!
//! THE DATA (`data/characters/locomotion.ron`): states, each a 1D blend space over one input (the
//! body's speed or its turn rate) placing clips at points along it, and transitions between states
//! when their conditions hold, each with a crossfade time. The shipped graph is idle, turning on the
//! spot (turn left, idle, turn right over the turn rate) and moving (walk and run over the speed);
//! a new state or clip is a row, not code.
//!
//! PLAYING A BLEND SPACE: the input picks the two neighbouring clips and the weight between them.
//! A `sync` space plays its clips on one shared phase (a walk and a run blended mid-stride put the
//! same foot down at the same moment, the standard fix for a blended gait's tangled legs), and a
//! `stride` space plays faster or slower so the clip's own speed matches the body's real speed
//! within limits, and backwards when the body moves backwards (stride warping's first form: feet
//! that slide less). A space without `sync` runs each clip on its own clock.
//!
//! TRANSITIONS: checked in order from the current state; the first whose conditions all hold
//! starts a crossfade of `blend_s` seconds, the old state still playing underneath while it fades
//! (inertialized transitions replace the crossfade in rung 3).
//!
//! The capsule is the truth (plan 3.3: "Gameplay never waits for animation"): nothing here moves the
//! body. The inputs are read from where the body went (`motion`).

use serde::Deserialize;

use crate::clip::ClipSet;
use crate::pose::{blend_into, blend_toward, Pose};
use crate::{HumanoidDef, Skeleton};

/// What a blend space or a condition reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Param {
    /// Metres a second along the way the body faces (negative backwards).
    Speed,
    /// Radians a second, positive turning left (counter-clockwise seen from above).
    TurnRate,
}

/// What a condition asks of its input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Test {
    Above,
    Below,
    AbsAbove,
    AbsBelow,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Condition {
    pub param: Param,
    pub test: Test,
    pub value: f32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SampleDef {
    pub at: f32,
    pub clip: String,
}

/// Stride warping's limits: the slowest and fastest a clip may be played to match the body's speed.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct StrideDef {
    pub min_rate: f32,
    pub max_rate: f32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StateDef {
    pub id: String,
    /// The input the blend space runs along (`|value|` for the speed, so a backwards walk plays the
    /// walk; see `stride`).
    pub param: Param,
    pub samples: Vec<SampleDef>,
    #[serde(default)]
    pub sync: bool,
    #[serde(default)]
    pub stride: Option<StrideDef>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TransitionDef {
    /// The states it leaves from; `"*"` for any.
    pub from: Vec<String>,
    pub to: String,
    pub when: Vec<Condition>,
    pub blend_s: f32,
}

/// `data/characters/locomotion.ron`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct GraphDef {
    pub id: String,
    pub initial: String,
    /// How fast the inputs follow the body's real motion, seconds (an exponential follow): long
    /// enough that a remote player's 15 updates a second read as one smooth speed.
    pub smoothing_s: f32,
    /// Faster than this, metres a second, a move is a teleport (a respawn, a scripted park), not a
    /// stride: the inputs are reset instead of reading a sprint.
    pub teleport_mps: f32,
    pub states: Vec<StateDef>,
    pub transitions: Vec<TransitionDef>,
}

/// A state with its clips resolved to indices.
#[derive(Debug, Clone, PartialEq)]
struct State {
    param: Param,
    /// (point along the input, clip index), sorted by the point.
    samples: Vec<(f32, usize)>,
    sync: bool,
    stride: Option<StrideDef>,
}

#[derive(Debug, Clone, PartialEq)]
struct Transition {
    from: Option<Vec<usize>>,
    to: usize,
    when: Vec<Condition>,
    blend_s: f32,
}

/// A graph checked against a body's clips.
#[derive(Debug, Clone, PartialEq)]
pub struct Graph {
    pub id: String,
    pub smoothing_s: f32,
    pub teleport_mps: f32,
    pub state_ids: Vec<String>,
    initial: usize,
    states: Vec<State>,
    transitions: Vec<Transition>,
}

impl Graph {
    pub fn parse(text: &str) -> Result<GraphDef, String> {
        crate::parse_ron(text, "locomotion graph")
    }

    /// Resolve every clip and state the graph names; an unknown one is an error naming it.
    pub fn resolve(def: &GraphDef, clips: &ClipSet) -> Result<Self, String> {
        let state_ids: Vec<String> = def.states.iter().map(|s| s.id.clone()).collect();
        let state_index = |id: &str| state_ids.iter().position(|s| s == id);
        let mut states = Vec::new();
        for s in &def.states {
            if s.samples.is_empty() {
                return Err(format!("locomotion graph {}: state {} has no clips", def.id, s.id));
            }
            let mut samples = Vec::new();
            for smp in &s.samples {
                let c = clips.index(&smp.clip).ok_or_else(|| format!("locomotion graph {}: state {} plays clip {}, which is not in data/characters/clips.ron", def.id, s.id, smp.clip))?;
                samples.push((smp.at, c));
            }
            samples.sort_by(|a, b| a.0.total_cmp(&b.0));
            states.push(State { param: s.param, samples, sync: s.sync, stride: s.stride });
        }
        let mut transitions = Vec::new();
        for t in &def.transitions {
            let to = state_index(&t.to).ok_or_else(|| format!("locomotion graph {}: a transition goes to {}, which is not a state", def.id, t.to))?;
            let from = if t.from.iter().any(|f| f == "*") {
                None
            } else {
                Some(
                    t.from
                        .iter()
                        .map(|f| state_index(f).ok_or_else(|| format!("locomotion graph {}: a transition leaves {f}, which is not a state", def.id)))
                        .collect::<Result<Vec<_>, _>>()?,
                )
            };
            transitions.push(Transition { from, to, when: t.when.clone(), blend_s: t.blend_s.max(0.0) });
        }
        let initial = state_index(&def.initial).ok_or_else(|| format!("locomotion graph {}: the initial state {} is not a state", def.id, def.initial))?;
        Ok(Graph { id: def.id.clone(), smoothing_s: def.smoothing_s, teleport_mps: def.teleport_mps, state_ids, initial, states, transitions })
    }

    pub fn state_index(&self, id: &str) -> Option<usize> {
        self.state_ids.iter().position(|s| s == id)
    }
}

/// What the body is doing, read from its motion (`motion::MotionTracker`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LocomotionInput {
    /// Metres a second along the facing, negative backwards.
    pub speed: f32,
    /// Metres a second to the side (positive to the body's left); kept for rung 3's strafing.
    pub side: f32,
    /// Radians a second, positive turning left.
    pub turn_rate: f32,
}

impl LocomotionInput {
    fn get(&self, p: Param) -> f32 {
        match p {
            Param::Speed => self.speed,
            Param::TurnRate => self.turn_rate,
        }
    }
}

/// Where one state is in its clips.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Playhead {
    /// 0 to 1 through a synchronised space.
    phase: f32,
    /// Seconds into an unsynchronised one.
    time: f32,
}

/// One character's place in the graph: the state it is in, the one fading out under it, and where
/// each is in its clips. State the character carries from frame to frame (like a position), not a
/// reuse of worked-out data.
#[derive(Debug, Clone, PartialEq)]
pub struct Animator {
    current: usize,
    head: Playhead,
    /// The state fading out, its playhead, the seconds the fade has run and its length.
    fading: Option<(usize, Playhead, f32, f32)>,
    scratch_a: Pose,
    scratch_b: Pose,
    scratch_c: Pose,
}

impl Animator {
    pub fn new(graph: &Graph) -> Self {
        Animator {
            current: graph.initial,
            head: Playhead::default(),
            fading: None,
            scratch_a: Pose::default(),
            scratch_b: Pose::default(),
            scratch_c: Pose::default(),
        }
    }

    /// Start a character partway through its clips, so a crowd does not step in unison.
    pub fn with_offset(graph: &Graph, fraction: f32, seconds: f32) -> Self {
        let mut a = Animator::new(graph);
        a.head = Playhead { phase: fraction.rem_euclid(1.0), time: seconds.max(0.0) };
        a
    }

    /// The state it is in, by index into `Graph::state_ids`.
    pub fn state(&self) -> usize {
        self.current
    }

    /// Is a crossfade running?
    pub fn is_fading(&self) -> bool {
        self.fading.is_some()
    }

    /// One frame: take a transition if one fires, move the playheads on by `dt`, and write the
    /// pose into `out` (twist joints included).
    pub fn update(
        &mut self,
        graph: &Graph,
        clips: &ClipSet,
        skel: &Skeleton,
        humanoid: &HumanoidDef,
        input: &LocomotionInput,
        dt: f32,
        out: &mut Pose,
    ) {
        let dt = if dt.is_finite() { dt.clamp(0.0, 0.25) } else { 0.0 };
        if let Some(t) = graph.transitions.iter().find(|t| {
            t.to != self.current
                && t.from.as_ref().map_or(true, |f| f.contains(&self.current))
                && t.when.iter().all(|c| holds(c, input))
        }) {
            let old = (self.current, self.head);
            self.current = t.to;
            self.head = Playhead::default();
            // A sync state entered from a sync state keeps the stride's phase.
            if graph.states[old.0].sync && graph.states[t.to].sync {
                self.head.phase = old.1.phase;
            }
            self.fading = if t.blend_s > 0.0 { Some((old.0, old.1, 0.0, t.blend_s)) } else { None };
        }
        advance(&graph.states[self.current], clips, input, dt, &mut self.head);
        let mut cur = std::mem::take(&mut self.scratch_a);
        evaluate(&graph.states[self.current], clips, skel, input, self.head, &mut cur, &mut self.scratch_c);
        match self.fading.as_mut() {
            Some((s, head, elapsed, len)) => {
                *elapsed += dt;
                let w = (*elapsed / *len).clamp(0.0, 1.0);
                advance(&graph.states[*s], clips, input, dt, head);
                let mut old = std::mem::take(&mut self.scratch_b);
                evaluate(&graph.states[*s], clips, skel, input, *head, &mut old, &mut self.scratch_c);
                blend_into(&old, &cur, smoothstep(w), out);
                self.scratch_b = old;
                if w >= 1.0 {
                    self.fading = None;
                }
            }
            None => {
                out.joints.clear();
                out.joints.extend_from_slice(&cur.joints);
            }
        }
        self.scratch_a = cur;
        crate::retarget::apply_twists(skel, humanoid, out);
    }
}

fn holds(c: &Condition, input: &LocomotionInput) -> bool {
    let v = input.get(c.param);
    match c.test {
        Test::Above => v > c.value,
        Test::Below => v < c.value,
        Test::AbsAbove => v.abs() > c.value,
        Test::AbsBelow => v.abs() < c.value,
    }
}

fn smoothstep(w: f32) -> f32 {
    w * w * (3.0 - 2.0 * w)
}

/// The two clips around `x` and the weight toward the upper one.
pub fn bracket(samples: &[(f32, usize)], x: f32) -> (usize, usize, f32) {
    let last = samples.len() - 1;
    if x <= samples[0].0 {
        return (0, 0, 0.0);
    }
    if x >= samples[last].0 {
        return (last, last, 0.0);
    }
    let i = samples.windows(2).position(|w| x >= w[0].0 && x <= w[1].0).unwrap_or(last - 1);
    let span = (samples[i + 1].0 - samples[i].0).max(1e-6);
    (i, i + 1, ((x - samples[i].0) / span).clamp(0.0, 1.0))
}

/// The blend space's position along its input: the speed's size (a backwards walk plays the walk,
/// backwards), the turn rate as it is.
fn space_input(s: &State, input: &LocomotionInput) -> f32 {
    match s.param {
        Param::Speed => input.speed.abs(),
        Param::TurnRate => input.turn_rate,
    }
}

/// The playback rate of a stride-warped space: the body's speed over the blended clips' own speed,
/// within the limits, negative when the body moves backwards. 1 for spaces without stride.
pub fn stride_rate(stride: Option<StrideDef>, body_speed: f32, clip_speed: f32) -> f32 {
    let Some(st) = stride else { return 1.0 };
    if clip_speed <= 1e-3 {
        return 1.0;
    }
    let r = (body_speed.abs() / clip_speed).clamp(st.min_rate, st.max_rate);
    if body_speed < 0.0 {
        -r
    } else {
        r
    }
}

fn advance(s: &State, clips: &ClipSet, input: &LocomotionInput, dt: f32, head: &mut Playhead) {
    let (a, b, w) = bracket(&s.samples, space_input(s, input));
    let (ca, cb) = (clips.get(s.samples[a].1), clips.get(s.samples[b].1));
    let rate = stride_rate(s.stride, input.speed, ca.speed_mps + (cb.speed_mps - ca.speed_mps) * w);
    if s.sync {
        let dur = (ca.duration + (cb.duration - ca.duration) * w).max(1e-3);
        head.phase = (head.phase + dt * rate / dur).rem_euclid(1.0);
    } else {
        head.time += dt * rate.abs();
    }
}

fn evaluate(s: &State, clips: &ClipSet, skel: &Skeleton, input: &LocomotionInput, head: Playhead, out: &mut Pose, scratch: &mut Pose) {
    let (a, b, w) = bracket(&s.samples, space_input(s, input));
    let (ca, cb) = (clips.get(s.samples[a].1), clips.get(s.samples[b].1));
    if s.sync {
        ca.sample_phase(skel, head.phase, out);
        if a != b && w > 0.0 {
            cb.sample_phase(skel, head.phase, scratch);
            blend_toward(out, scratch, w);
        }
    } else {
        ca.sample(skel, head.time, out);
        if a != b && w > 0.0 {
            cb.sample(skel, head.time, scratch);
            blend_toward(out, scratch, w);
        }
    }
}

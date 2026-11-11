//! FEET ON THE WORLD (character animation rung 2a, 2026-11-09; docs/design/character-animation.md
//! section 4, "Feet and hands on the world").
//!
//! A clip is made on a flat floor. The world is not flat: stair treads, ramps, a deck's edge, a
//! hillside, a road bed cut into a slope. Rung 1 drew every clip as it came, so the feet sank up to
//! about 5 cm in the walk cycle (sprint deepest: the retargeted planted foot sits under the clip
//! rig's floor) and hovered over or sank through stair treads. This module puts each foot on the
//! ground under it, every frame, after the locomotion graph has made its pose:
//!
//! 1. THE GROUND under each foot comes from the engine through [`Ground`] (aboard: the footing that
//!    already answers "where is the floor under a body", asked per sole point; on a planet: the
//!    terrain sampler with positions and directions in f64, the map packs' road beds and building
//!    pads included). This crate never reads the world itself (it is pure: no engine, no GPU).
//! 2. CONTACTS are labelled on every clip when it is baked onto a body ([`label_contacts`]): a foot
//!    is planted in a frame when its sole is at the clip's floor and not rising or falling. The
//!    graph blends them like poses (`Animator::contacts`).
//! 3. PER FOOT, two sole points (under the ankle and under the ball of the foot, each its rest
//!    height under its joint, turned with the joint) are measured against the ground under them. A
//!    planted foot's lower sole point is put ON the ground (so the clip's sinking is gone and a foot
//!    rests on a tread, not in it); a swinging foot keeps the clip's lift, over the real ground
//!    under it. Whichever sole point meets its ground is the pivot (the toe pivot: on a step's edge
//!    the ball rests and the heel hangs), and a planted foot turns to the surface's slope about it.
//! 4. FOOT LOCKING: when a foot plants, the spot is kept in the world until it lifts, and the leg
//!    reaches it, so a planted foot never slides however the body's speed and the clip's stride
//!    disagree; it lets go smoothly when the clip lifts it, or at once when the body has gone too far
//!    from it (a teleport, a turn the clip does not follow).
//! 5. STAIRS: when a foot plants on a stair tread, the spot is moved along the stair (from the stair
//!    piece's own riser and tread, through [`StepInfo`]) so the foot lands on the tread's middle,
//!    not across its nosing.
//! 6. THE PELVIS drops by the larger drop (or rises, within limits, when both feet stand higher than
//!    the body's root: a remote player's root interpolated between two treads), smoothed, so the
//!    lower foot can reach its ground with the knee still bent; then a TWO-BONE ANALYTIC IK per leg
//!    (thigh and shin, in the plane the clip's knee already bends in) puts each ankle exactly where
//!    its foot must be. Full-body IK is a later rung (plan section 4).
//!
//! Everything works on each person's own reshaped skeleton (`Skeleton::reshaped`), never the body's
//! unshaped one, and in the body's model space (its feet at y = 0, +Y up, +Z its front), so a tall
//! or a short person's legs bend where their legs are.
//!
//! THE CAPSULE IS THE TRUTH (plan 3.3): nothing here moves the person. Their position, the floor
//! they stand on, what they can reach, all stay the gameplay systems'; this only bends the drawn
//! legs onto the ground those systems already stand the body on.
//!
//! MODES (plan 8.6, the dual-mode rule): `Full` is all of the above (the near tier in the realistic
//! animation mode); `Pelvis` is only the pelvis offset, no leg IK, no locking, one ground query per
//! foot (the middle tier, and every body in the simplified animation mode); `Off` is rung 1's pose
//! as it came (the A/B switch the rigs measure against).

use glam::{Mat4, Quat, Vec3};
use serde::Deserialize;

use crate::clip::Clip;
use crate::pose::{model_matrices, Pose};
use crate::{HumanoidDef, Skeleton};

/// What the ground is under a point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundHit {
    /// The ground's point on the vertical through the asked point, world metres.
    pub point: Vec3,
    /// The surface's up, world, unit length.
    pub normal: Vec3,
    /// The stair tread under the point, when it is one.
    pub step: Option<StepInfo>,
}

/// A stair tread, from the stair piece's own data (its riser and tread): what the footstep planner
/// needs to put a foot on the tread's middle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepInfo {
    /// Up the stair, world, horizontal, unit length.
    pub axis: Vec3,
    /// The tread's middle, as a distance along `axis` (`point.dot(axis)` of a world point on it).
    pub centre: f32,
    /// How deep the tread is along the axis, metres.
    pub depth: f32,
}

/// The ground under a person, as the engine knows it. Asked a few times a frame per near body (two
/// sole points a foot, plus their slope), once a foot for a middle one.
pub trait Ground {
    /// The ground under the world point `at`: the floor a body standing there would stand on (the
    /// engine knows which floors are within a step of the person). None where there is nothing to
    /// stand on; that foot is then left as the clip has it.
    fn ground(&self, at: Vec3) -> Option<GroundHit>;
}

/// How much of the feet's work a body gets (plan 8.6 and 8.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FootMode {
    /// The pose as the clip made it (rung 1): the rigs' A/B arm.
    Off,
    /// Only the pelvis offset: the whole body raised or lowered onto the ground under its feet, no
    /// leg IK and no locking (the middle tier, and the simplified animation mode).
    Pelvis,
    /// Pelvis, two-bone leg IK, the foot turned to the slope with a toe pivot, foot locking and
    /// stair planning (the near tier in the realistic animation mode).
    Full,
}

fn d_max_drop() -> f32 {
    0.45
}
fn d_max_raise() -> f32 {
    0.25
}
fn d_pelvis_follow() -> f32 {
    0.07
}
fn d_lift_follow() -> f32 {
    0.06
}
fn d_lock_out() -> f32 {
    0.15
}
fn d_lock_break() -> f32 {
    0.35
}
fn d_tilt() -> f32 {
    35.0
}
fn d_contact_rel() -> f32 {
    0.04
}
fn d_contact_abs() -> f32 {
    0.08
}
fn d_contact_rise() -> f32 {
    0.5
}
fn d_slope_probe() -> f32 {
    0.08
}
fn d_true() -> bool {
    true
}
fn d_tread_margin() -> f32 {
    0.035
}
fn d_swing_ahead() -> f32 {
    0.12
}
fn d_full_budget() -> f32 {
    1.0
}
fn d_contact_slip() -> f32 {
    0.3
}
fn d_contact_slip_share() -> f32 {
    0.35
}

/// The feet's numbers: `data/characters/characters.ron`'s `feet` (data, so a modder tunes them and
/// the tests read the shipped ones). Every field has the value it shipped with as its default.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct FeetDef {
    /// The furthest the pelvis drops for a foot on lower ground, metres (a stair's riser and a half
    /// is 0.28; past this the leg would straighten and the foot float).
    #[serde(default = "d_max_drop")]
    pub max_drop_m: f32,
    /// The furthest it rises when both feet stand higher than the root, metres.
    #[serde(default = "d_max_raise")]
    pub max_raise_m: f32,
    /// How fast the pelvis follows, seconds (an exponential follow), so a root that steps up a tread
    /// at a time moves the hips smoothly.
    #[serde(default = "d_pelvis_follow")]
    pub pelvis_follow_s: f32,
    /// How fast a swinging foot's lift follows the ground under it, seconds, so a foot crossing over
    /// a tread's edge rises in a few frames rather than in one (a planted foot follows at once).
    #[serde(default = "d_lift_follow")]
    pub lift_follow_s: f32,
    /// How long a lock takes to let go once the clip lifts the foot, seconds. (A lock holds fully
    /// from the frame a foot plants: the stair footstep is eased in during the swing instead.)
    #[serde(default = "d_lock_out")]
    pub lock_out_s: f32,
    /// A lock breaks at once when the clip's foot is further than this from it, metres.
    #[serde(default = "d_lock_break")]
    pub lock_break_m: f32,
    /// The most a planted foot turns to a slope, degrees.
    #[serde(default = "d_tilt")]
    pub max_tilt_deg: f32,
    /// Labelling: a sole point within this of the clip's lowest for that foot is on the floor, metres.
    #[serde(default = "d_contact_rel")]
    pub contact_rel_m: f32,
    /// Labelling: and never higher than this over the body's floor, metres (so a jump's tucked feet,
    /// low only against themselves, are never planted).
    #[serde(default = "d_contact_abs")]
    pub contact_abs_m: f32,
    /// Labelling: and rising or falling slower than this, metres a second.
    #[serde(default = "d_contact_rise")]
    pub contact_rise_mps: f32,
    /// Labelling: and sliding no further from how a planted foot moves in a clip played in place
    /// (backwards at the clip's speed) than this, metres a second, plus `contact_slip_share` of the
    /// clip's speed (a run's planted foot is less even than a walk's).
    #[serde(default = "d_contact_slip")]
    pub contact_slip_mps: f32,
    #[serde(default = "d_contact_slip_share")]
    pub contact_slip_share: f32,
    /// How far apart the ground is sampled to read its slope, metres. A step bigger than a slope
    /// over that span is a riser, not a slope, and the foot stays level.
    #[serde(default = "d_slope_probe")]
    pub slope_probe_m: f32,
    /// Plan footsteps on stairs: a foot planting on a stair tread is moved along the stair onto the
    /// tread's middle (from the stair piece's own riser and tread).
    #[serde(default = "d_true")]
    pub plan_steps: bool,
    /// The least room left between the end of a planted foot that points up a stair and the next
    /// step's riser, metres (the other end may hang over the nosing behind).
    #[serde(default = "d_tread_margin")]
    pub tread_margin_m: f32,
    /// How far ahead of its toe tips a swinging foot reads the ground, metres, so its lift rises
    /// before the next step instead of popping up at the riser.
    #[serde(default = "d_swing_ahead")]
    pub swing_ahead_m: f32,
    /// The most a frame spends on full foot solves, milliseconds: the engine gives the full solve to
    /// as many of the nearest bodies as this buys at what a solve has been costing, and the pelvis
    /// offset to the rest of the near tier (question 5: full realism where the measured budget
    /// allows, simplified automatically below it).
    #[serde(default = "d_full_budget")]
    pub full_budget_ms: f32,
}

impl Default for FeetDef {
    fn default() -> Self {
        FeetDef {
            max_drop_m: d_max_drop(),
            max_raise_m: d_max_raise(),
            pelvis_follow_s: d_pelvis_follow(),
            lift_follow_s: d_lift_follow(),
            lock_out_s: d_lock_out(),
            lock_break_m: d_lock_break(),
            max_tilt_deg: d_tilt(),
            contact_rel_m: d_contact_rel(),
            contact_abs_m: d_contact_abs(),
            contact_rise_mps: d_contact_rise(),
            contact_slip_mps: d_contact_slip(),
            contact_slip_share: d_contact_slip_share(),
            slope_probe_m: d_slope_probe(),
            plan_steps: true,
            tread_margin_m: d_tread_margin(),
            swing_ahead_m: d_swing_ahead(),
            full_budget_ms: d_full_budget(),
        }
    }
}

/// One leg's joints by role.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Leg {
    pub upper: usize,
    pub lower: usize,
    pub foot: usize,
    pub toes: usize,
}

/// A skeleton's legs and soles: which joints the IK bends (`leftUpperLeg -> leftLowerLeg ->
/// leftFoot -> leftToes` and the right side, MakeHuman's `upperleg01 -> lowerleg01 -> foot ->
/// toe3-1`), and how high each sole point stands under its joint at rest (every rig stands its feet
/// flat at rest at y = 0: rung 1's lesson, so the feet are never aligned across rigs).
#[derive(Debug, Clone, PartialEq)]
pub struct FootRig {
    pub hips: usize,
    pub legs: [Leg; 2],
    /// Each foot's ankle (the foot joint) height at rest, metres: the sole under the ankle.
    pub heel_h: [f32; 2],
    /// Each foot's ball (the toes joint) height at rest, metres: the sole under the ball.
    pub ball_h: [f32; 2],
    /// How far each foot reaches behind its heel sole point and ahead of its ball sole point at
    /// rest, metres (the back of the heel, the tips of the toes), measured from the mesh when the
    /// body loads (`with_extents`); the footstep planner fits the whole foot on a tread with them.
    pub reach: [(f32, f32); 2],
    /// Each foot's heel sole point to its ball sole point at rest, along the ground, metres: with
    /// `reach`, the flat foot's whole length, which the stair planner fits on a tread.
    pub heel_to_ball: [f32; 2],
    /// How far any of each foot's vertices stands ahead of its toe tip sole point with the toes
    /// pitched by -90 to 90 degrees in 5 degree steps (index 18 is level), metres, measured from the
    /// mesh by `with_extents`: the toe cap's front is a few centimetres above the sole, so a foot
    /// pitched toes-down carries it ahead of the toe tips, and swinging over a nosing it went into
    /// the riser above while every sole point was clear (the homestead stair test read 9.4 cm
    /// deep, the front vertex 1.4 cm ahead of the toe tip point). A swinging foot reads the ground
    /// this far ahead of its toe tips (`toe_ahead_at`), at the pitch the toes have that frame.
    pub toe_ahead: [[f32; TOE_PITCHES]; 2],
    /// The same behind the back of the heel, with the foot (the foot joint) pitched: the heel
    /// counter stands above the sole, so a foot pitched toes-up carries it behind the heel's back
    /// point, and walking DOWN the homestead stair it cut 3.7 to 6.3 cm into the riser of the step
    /// the foot was leaving (the test's whole round, the vertex 1.7 to 2.6 cm behind that point).
    pub heel_behind: [[f32; TOE_PITCHES]; 2],
    /// Each joint's model-space rest rotation, for the sole points' turn.
    rest_rot: Vec<Quat>,
}

/// A foot's reach behind its heel and ahead of its ball when no mesh says (metres).
const DEFAULT_REACH: (f32, f32) = (0.06, 0.06);

/// The toe pitches `FootRig::toe_ahead` is measured at: -90 to 90 degrees in 5 degree steps (a walk
/// swings its toes down as far as 70 degrees from their rest).
pub const TOE_PITCHES: usize = 37;
const TOE_PITCH_STEP_DEG: f32 = 5.0;

/// A swinging foot this high over its floor (or higher) carries none of its planned stair footstep;
/// touching down it carries all of it (metres).
const PLAN_EASE_M: f32 = 0.12;

impl FootRig {
    /// The legs of a skeleton, or None when a leg joint has no bone (such a body is drawn as its
    /// clips make it, without feet on the world).
    pub fn new(skel: &Skeleton, humanoid: &HumanoidDef) -> Option<Self> {
        let j = |r: &str| skel.joint_by_role(humanoid, r);
        let leg = |side: &str| -> Option<Leg> {
            Some(Leg {
                upper: j(&format!("{side}UpperLeg"))?,
                lower: j(&format!("{side}LowerLeg"))?,
                foot: j(&format!("{side}Foot"))?,
                toes: j(&format!("{side}Toes"))?,
            })
        };
        let legs = [leg("left")?, leg("right")?];
        let h = |k: usize| skel.rest_position(k).y.max(0.0);
        let along = |l: &Leg| {
            let d = skel.rest_position(l.toes) - skel.rest_position(l.foot);
            Vec3::new(d.x, 0.0, d.z).length()
        };
        Some(FootRig {
            hips: j("hips")?,
            heel_h: [h(legs[0].foot), h(legs[1].foot)],
            ball_h: [h(legs[0].toes), h(legs[1].toes)],
            reach: [DEFAULT_REACH; 2],
            heel_to_ball: [along(&legs[0]), along(&legs[1])],
            toe_ahead: [[0.0; TOE_PITCHES]; 2],
            heel_behind: [[0.0; TOE_PITCHES]; 2],
            legs,
            rest_rot: (0..skel.joint_count()).map(|k| skel.rest_world_rotation(k)).collect(),
        })
    }

    /// This rig with each foot's reach measured from its vertices at rest (model space, the body
    /// facing +Z): the back of the heel behind the ankle, the toe tips ahead of the ball.
    pub fn with_extents(mut self, skel: &Skeleton, feet: &[Vec<Vec3>; 2]) -> Self {
        for (i, verts) in feet.iter().enumerate() {
            if verts.is_empty() {
                continue;
            }
            let heel_z = skel.rest_position(self.legs[i].foot).z;
            let ball_z = skel.rest_position(self.legs[i].toes).z;
            let min_z = verts.iter().map(|v| v.z).fold(f32::INFINITY, f32::min);
            let max_z = verts.iter().map(|v| v.z).fold(f32::NEG_INFINITY, f32::max);
            self.reach[i] = ((heel_z - min_z).max(0.0), (max_z - ball_z).max(0.0));
            // The toe cap ahead of the toe tip point as the toes pitch (about the ball, the toes
            // joint; the difference between two points of one rigid foot does not depend on the
            // pivot), every 5 degrees to 90 either way.
            let ball = skel.rest_position(self.legs[i].toes);
            let tip = Vec3::new(ball.x, ball.y - self.ball_h[i], max_z);
            let half = (TOE_PITCHES / 2) as f32;
            for (k, slot) in self.toe_ahead[i].iter_mut().enumerate() {
                let turn = Quat::from_rotation_x(((k as f32 - half) * TOE_PITCH_STEP_DEG).to_radians());
                let tip_z = (turn * (tip - ball)).z;
                *slot = verts.iter().map(|v| (turn * (*v - ball)).z - tip_z).fold(0.0_f32, f32::max);
            }
            // And the heel counter behind the heel's back point as the foot pitches (about the
            // ankle, the foot joint).
            let ankle = skel.rest_position(self.legs[i].foot);
            let back = Vec3::new(ankle.x, ankle.y - self.heel_h[i], min_z);
            for (k, slot) in self.heel_behind[i].iter_mut().enumerate() {
                let turn = Quat::from_rotation_x(((k as f32 - half) * TOE_PITCH_STEP_DEG).to_radians());
                let back_z = (turn * (back - ankle)).z;
                *slot = verts.iter().map(|v| back_z - (turn * (*v - ankle)).z).fold(0.0_f32, f32::max);
            }
        }
        self
    }

    /// This rig with each foot's sole heights measured from its vertices at rest (model space), and
    /// how far the lowest of them stands below the body's floor (y = 0), metres: the LIFT (2026-11-09,
    /// "the look"; rung 2a's lesson for the clothes: "a SHOE with a sole of its own changes it"). The
    /// heel's sole is the lowest vertex behind the middle of the foot, the ball's the lowest ahead of
    /// it, each measured from where the person is drawn: `lift` higher, so a shoe's sole, not the foot
    /// inside it, stands on the floor. A bare foot's soles come out at its own lowest vertices (a few
    /// millimetres from its joint heights). Measure before `with_extents`, which reads the soles.
    pub fn with_soles(mut self, skel: &Skeleton, feet: &[Vec<Vec3>; 2]) -> (Self, f32) {
        let low = feet.iter().flatten().map(|v| v.y).fold(f32::INFINITY, f32::min);
        if !low.is_finite() {
            return (self, 0.0);
        }
        let lift = (-low).max(0.0);
        for (i, verts) in feet.iter().enumerate() {
            let ankle = skel.rest_position(self.legs[i].foot);
            let ball = skel.rest_position(self.legs[i].toes);
            let mid = 0.5 * (ankle.z + ball.z);
            let lowest = |behind: bool| verts.iter().filter(|v| (v.z <= mid) == behind).map(|v| v.y).fold(f32::INFINITY, f32::min);
            let (heel, front) = (lowest(true), lowest(false));
            if heel.is_finite() {
                self.heel_h[i] = (ankle.y - heel).max(0.0);
            }
            if front.is_finite() {
                self.ball_h[i] = (ball.y - front).max(0.0);
            }
        }
        (self, lift)
    }

    /// This rig (a person's own, on their reshaped skeleton) with what `with_extents` measured from
    /// the body's mesh copied from `body`'s rig: the feet's reach, toe cap and heel counter. Every
    /// field measured from the mesh is copied here, so a new one cannot be forgotten at a call site
    /// (the engine copied only the reach and every person in the game went without the toe cap
    /// and the heel counter).
    pub fn with_measured(mut self, body: &FootRig) -> Self {
        self.reach = body.reach;
        self.toe_ahead = body.toe_ahead;
        self.heel_behind = body.heel_behind;
        self
    }

    /// The table slot for joint `k`'s pitch in `model` (its forward axis turned from rest).
    fn pitch_slot(&self, k: usize, model: &[Mat4]) -> usize {
        let (_, r, _) = model[k].to_scale_rotation_translation();
        let f = (r * self.rest_rot[k].inverse()).normalize() * Vec3::Z;
        // `Quat::from_rotation_x(a)` turns +Z to (0, -sin a, cos a).
        let pitch = (-f.y).atan2(Vec3::new(f.x, 0.0, f.z).length()).to_degrees();
        let half = (TOE_PITCHES / 2) as f32;
        (pitch / TOE_PITCH_STEP_DEG + half).round().clamp(0.0, (TOE_PITCHES - 1) as f32) as usize
    }

    /// The most of `table` within 15 degrees either side of joint `k`'s pitch: a vertex near the
    /// ankle or the ball is skinned partly to the next joint and bends further than the rigid foot
    /// the table is measured for (walking down the stair a heel counter vertex stood 1.7 cm behind
    /// the heel's back point where the table at the foot's own pitch said 0.85 cm).
    fn overhang(&self, table: &[f32; TOE_PITCHES], k: usize, model: &[Mat4]) -> f32 {
        let s = self.pitch_slot(k, model);
        table[s.saturating_sub(3)..=(s + 3).min(TOE_PITCHES - 1)].iter().copied().fold(0.0, f32::max)
    }

    /// How far foot `i`'s toe cap stands ahead of its toe tip sole point near the pitch its toes
    /// have in `model` (`toe_ahead`), metres.
    pub fn toe_ahead_at(&self, i: usize, model: &[Mat4]) -> f32 {
        self.overhang(&self.toe_ahead[i], self.legs[i].toes, model)
    }

    /// How far foot `i`'s heel counter stands behind its heel's back sole point near the pitch the
    /// foot has in `model` (`heel_behind`), metres.
    pub fn heel_behind_at(&self, i: usize, model: &[Mat4]) -> f32 {
        self.overhang(&self.heel_behind[i], self.legs[i].foot, model)
    }

    /// The two sole points of foot `i` (under the ankle, under the ball), model space, for a posed
    /// skeleton's model matrices.
    pub fn soles(&self, i: usize, model: &[Mat4]) -> (Vec3, Vec3) {
        let s = self.sole_points(i, model);
        (s[1], s[2])
    }

    /// The four sole points of foot `i`, model space, for a posed skeleton's model matrices: the
    /// back of the heel and the point under the ankle (turned with the foot), the point under the
    /// ball and the toe tips (turned with the toes). A foot pitched up at a heel strike meets the
    /// floor at the back of its heel, one flexed onto its toes at their tips, so all four are what
    /// "the sole is on the ground" is measured by.
    pub fn sole_points(&self, i: usize, model: &[Mat4]) -> [Vec3; 4] {
        let l = self.legs[i];
        let (back, front) = self.reach[i];
        let point = |k: usize, offset: Vec3| -> Vec3 {
            let (_, r, t) = model[k].to_scale_rotation_translation();
            let turn = (r * self.rest_rot[k].inverse()).normalize();
            t + turn * offset
        };
        let (hh, bh) = (self.heel_h[i], self.ball_h[i]);
        [
            point(l.foot, Vec3::new(0.0, -hh, -back)),
            point(l.foot, Vec3::new(0.0, -hh, 0.0)),
            point(l.toes, Vec3::new(0.0, -bh, 0.0)),
            point(l.toes, Vec3::new(0.0, -bh, front)),
        ]
    }
}

/// Label each frame of a clip baked on `skel` with which feet are planted (`Clip::contacts`). A foot
/// is planted when its lower sole point is within `contact_rel_m` of the lowest it goes in this
/// clip and no higher than `contact_abs_m` over the floor, and it moves the way a planted foot moves
/// in a clip played in place: backwards under the body at the clip's own speed (`speed_mps`; still,
/// for a clip that stands), within `contact_slip_mps` plus a share of that speed, and rising or
/// falling slower than `contact_rise_mps`. A clip with `air` set (the jump's air loop, `clips.ron`'s
/// `air` tag) has no planted frame: its hanging feet are low only against themselves. A planted
/// stretch one frame long is dropped.
pub fn label_contacts(clip: &mut Clip, skel: &Skeleton, rig: &FootRig, def: &FeetDef, air: bool) {
    let n = clip.frames;
    if n == 0 {
        clip.contacts.clear();
        return;
    }
    if air {
        clip.contacts = vec![[0.0, 0.0]; n];
        return;
    }
    let fps = clip.fps.max(1.0);
    let mut pose = Pose::default();
    let mut model = Vec::new();
    // Each frame, each foot: its four sole points (model space).
    let mut soles = vec![[[Vec3::ZERO; 4]; 2]; n];
    for (f, s) in soles.iter_mut().enumerate() {
        clip.sample(skel, f as f32 / fps, &mut pose);
        model_matrices(skel, &pose, &mut model);
        for (i, p) in s.iter_mut().enumerate() {
            *p = rig.sole_points(i, &model);
        }
    }
    let lowest = |pts: &[Vec3; 4]| (0..4).min_by(|&a, &b| pts[a].y.total_cmp(&pts[b].y)).unwrap_or(1);
    // A planted foot in a clip played in place slides backwards (-Z, the body faces +Z) at the
    // clip's own speed; a run's stance is brief and bounces, so the limits grow with the speed.
    let planted_v = Vec3::new(0.0, 0.0, -clip.speed_mps);
    let slip = def.contact_slip_mps + def.contact_slip_share * clip.speed_mps;
    let rise_limit = def.contact_rise_mps + def.contact_slip_share * 0.3 * clip.speed_mps;
    let mut contacts = vec![[0.0_f32; 2]; n];
    for i in 0..2 {
        let floor = soles.iter().map(|s| s[i][lowest(&s[i])].y).fold(f32::INFINITY, f32::min);
        for f in 0..n {
            // The lowest sole point's own velocity, a central difference (wrapped in a loop,
            // one-sided at a held clip's ends).
            let (a, b) = if clip.looped && n > 2 {
                ((f + n - 2) % (n - 1), (f + 1) % (n - 1))
            } else {
                (f.saturating_sub(1), (f + 1).min(n - 1))
            };
            let span = if clip.looped && n > 2 { 2 } else { (b - a).max(1) };
            let k = lowest(&soles[f][i]);
            let v = (soles[b][i][k] - soles[a][i][k]) * fps / span as f32;
            let h = soles[f][i][k].y;
            let flat_v = Vec3::new(v.x, 0.0, v.z);
            let planted = h <= floor + def.contact_rel_m && h <= def.contact_abs_m && v.y.abs() <= rise_limit && (flat_v - planted_v).length() <= slip;
            contacts[f][i] = if planted { 1.0 } else { 0.0 };
        }
        // A gap of one or two frames inside a plant is noise too (a foot rolling from heel to ball
        // can lift its lowest point for a frame): closed, so one stance is one plant.
        let at = |c: &Vec<[f32; 2]>, k: isize| -> f32 {
            let k = if clip.looped { k.rem_euclid(n as isize) } else { k.clamp(0, n as isize - 1) };
            c[k as usize][i]
        };
        let before = contacts.clone();
        for f in 0..n as isize {
            if before[f as usize][i] > 0.5 {
                continue;
            }
            let held = |gap: isize| (1..=gap).any(|b| at(&before, f - b) > 0.5 && (1..=gap).any(|a| at(&before, f + a) > 0.5 && a + b <= gap + 1));
            if held(2) {
                contacts[f as usize][i] = 1.0;
            }
        }
        // A one-frame plant is noise.
        for f in 0..n {
            let prev = if f == 0 { if clip.looped { contacts[n - 1][i] } else { 0.0 } } else { contacts[f - 1][i] };
            let next = if f + 1 == n { if clip.looped { contacts[0][i] } else { 0.0 } } else { contacts[f + 1][i] };
            if contacts[f][i] > 0.5 && prev < 0.5 && next < 0.5 {
                contacts[f][i] = 0.0;
            }
        }
    }
    clip.contacts = contacts;
}

/// One foot's state from frame to frame (like a position: state, not reuse).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct FootState {
    /// Where the planted foot's contact point is kept, world, and how firmly (0 to 1).
    lock: Option<Vec3>,
    /// Which sole point the lock holds: 0 the heel's, 1 the ball's.
    lock_pivot: usize,
    lock_w: f32,
    /// The stair footstep planned for this swing: the stair's axis (world), the tread's middle along
    /// it (eased when the tread under the foot changes) and the move along it that puts the whole
    /// foot there, metres.
    plan: Option<(Vec3, f32, f32)>,
    /// The swing's share of that move this frame, model space.
    swing_plan: Vec3,
    /// The tread this swing will land on (axis, middle, depth), chosen while the foot is low and
    /// falling (`Some(None)`: chosen, and it is not a stair); None until then.
    landing: Option<Option<(Vec3, f32, f32)>>,
    /// Last frame's foot middle (world) and lift, for the landing projection.
    prev: Option<(Vec3, f32)>,
    planted: bool,
    /// The smoothed vertical correction of a swinging foot, model metres.
    lift: f32,
}

/// What one foot did this frame (the dev overlay and the foot probe read it).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FootReport {
    /// The clip's contact weight this frame (0 in the air to 1 planted).
    pub contact: f32,
    /// The ground found under the foot's pivot, world (None: nothing to stand on there).
    pub ground: Option<Vec3>,
    /// Where the ankle was put, world.
    pub ankle: Vec3,
    /// Kept in place this frame (locked more than half).
    pub locked: bool,
    /// Moved onto a stair tread's middle when it planted.
    pub on_tread: bool,
}

/// What the feet did this frame.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FeetReport {
    pub feet: [FootReport; 2],
    /// The pelvis offset applied, world metres along the body's up.
    pub pelvis_m: f32,
    /// Ground queries made.
    pub queries: u32,
}

/// One person's feet from frame to frame: the pelvis offset and each foot's lock. Kept per figure
/// beside its `Animator`, made again when the figure is (a teleport resets it: `reset`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FootPlacer {
    pelvis: f32,
    /// The offset the pelvis is heading for (held through a run's flight in Pelvis mode).
    pelvis_target: f32,
    feet: [FootState; 2],
    started: bool,
    /// The last frame's report.
    pub report: FeetReport,
}

/// The frame's inputs for one person.
#[derive(Debug, Clone, Copy)]
pub struct FootFrame {
    /// The body's model space to the world (its feet, its facing about its up, its scale).
    pub world: Mat4,
    /// The clip's contacts this frame (`Animator::contacts`), left then right.
    pub contacts: [f32; 2],
    /// Off the ground (a jump, a fall, a ladder): no foot is planted and the pelvis eases back.
    pub airborne: bool,
    pub dt: f32,
    /// The hips are held by an interaction's contact (rung 2b: seated, the hips on the seat): the
    /// pelvis is not moved for the feet, and the thighs stay on the seat as the clip lays them: each
    /// lower leg swings about its knee to put its foot on the floor when the floor is within the
    /// shin's reach (a low seat: the foot slides forward), and hangs toward it when it is not (a seat
    /// high for the body: the feet dangle). The operator, 2026-11-10: "Dangle otherwise the
    /// character's butt clips down through the seat or we stretch the character."
    pub pelvis_fixed: bool,
    /// Seated, plant the feet with the whole leg instead (the thighs slope down off the seat to reach
    /// the floor): the dev red twin, showcase `seat_feet` plant, as rung 2b first drew a high seat.
    pub seat_plant: bool,
}

impl FootPlacer {
    /// Forget the locks and the pelvis (a teleport, a respawn, a figure coming back into view).
    pub fn reset(&mut self) {
        *self = FootPlacer::default();
    }

    /// The pelvis offset now applied, world metres.
    pub fn pelvis(&self) -> f32 {
        self.pelvis
    }

    /// Put the feet on the ground: `pose` is the graph's pose on `skel` (the person's reshaped
    /// skeleton) and `model` its model matrices, both changed in place. Returns what it did (also
    /// kept as `report`).
    #[allow(clippy::too_many_arguments)]
    pub fn place(
        &mut self,
        rig: &FootRig,
        skel: &Skeleton,
        humanoid: &HumanoidDef,
        pose: &mut Pose,
        model: &mut Vec<Mat4>,
        frame: &FootFrame,
        ground: &dyn Ground,
        mode: FootMode,
        def: &FeetDef,
    ) -> FeetReport {
        let mut report = FeetReport::default();
        if mode == FootMode::Off || model.len() < skel.joint_count() {
            self.reset();
            self.report = report;
            return report;
        }
        let dt = if frame.dt.is_finite() { frame.dt.clamp(0.0, 0.25) } else { 0.0 };
        let follow = |secs: f32| if secs > 1e-4 { 1.0 - (-dt / secs).exp() } else { 1.0 };
        let to_model = frame.world.inverse();
        let scale = frame.world.x_axis.truncate().length().max(1e-4);
        let up_w = frame.world.transform_vector3(Vec3::Y).normalize_or_zero();
        let mut queries = 0u32;
        let mut ask = |p_model: Vec3| -> Option<GroundHit> {
            queries += 1;
            ground.ground(frame.world.transform_point3(p_model))
        };
        let contact = |i: usize| if frame.airborne { 0.0 } else { frame.contacts[i].clamp(0.0, 1.0) };

        // Each foot's wanted vertical change (model metres) and, in Full, its target transform.
        let mut dys = [0.0_f32; 2];
        let mut targets: [Option<(Vec3, Quat)>; 2] = [None, None];
        for i in 0..2 {
            let c = contact(i);
            let leg = rig.legs[i];
            let sole = rig.sole_points(i, model);
            let (heel, ball) = (sole[1], sole[2]);
            let ankle = model[leg.foot].w_axis.truncate();
            let lift = sole.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
            let st = &mut self.feet[i];
            if mode == FootMode::Pelvis {
                // One query, under the lowest sole point.
                let low = sole.iter().copied().min_by(|a, b| a.y.total_cmp(&b.y)).unwrap_or(heel);
                let Some(g) = ask(low) else { continue };
                let gy = to_model.transform_point3(g.point).y;
                let want = lift.max(0.0) * (1.0 - c);
                dys[i] = want - (lift - gy);
                report.feet[i] = FootReport { contact: c, ground: Some(g.point), ankle: frame.world.transform_point3(ankle), locked: false, on_tread: false };
                continue;
            }
            // FOOT LOCKING (horizontal): the planted foot's CONTACT POINT is kept where it touched
            // down: the heel's sole point at a heel strike, the ball's as the foot rolls onto it
            // (the lock moves from one to the other where they are, so nothing jumps), never the
            // ankle, which rises and travels forward as a planted foot rolls over its ball.
            let planted = c > 0.5;
            let clip_ankle_w = frame.world.transform_point3(ankle);
            let flat = |v: Vec3| v - up_w * v.dot(up_w);
            let soles_w = [frame.world.transform_point3(heel), frame.world.transform_point3(ball)];
            // Which sole point carries the foot in the clip: the lower, with 5 mm of hysteresis.
            let carrier = match st.lock_pivot {
                0 if ball.y < heel.y - 0.005 => 1,
                1 if heel.y < ball.y - 0.005 => 0,
                p if st.lock.is_some() => p,
                _ => usize::from(ball.y < heel.y),
            };
            // FOOTSTEPS ON STAIRS: while a foot swings over a stair tread, the move that would put
            // the whole foot (back of the heel to the toe tips) on that tread's middle is worked out
            // from the stair piece's own riser and tread, and eased in as the foot comes down (none
            // at the top of the swing, all of it by touchdown), so the foot lands on the tread's
            // middle and the lock keeps it there: never a jump at touchdown, never a slide after.
            // WHERE IT WILL LAND: once the swinging foot is low and coming down, its touchdown is
            // projected from how it is moving (its sideways speed over the ground, its fall toward
            // its floor), and the tread under that point is chosen (refined as it comes); the move that
            // centres the whole foot on that tread is worked out exactly from where the foot is now
            // each frame, so it neither lags the sweeping foot nor jumps when the swing crosses a
            // nosing. (A first build eased the tread's middle instead; it lagged the last nosing and
            // landed feet 6 to 7 cm behind the middle, heels over the tread below.)
            let mid = (heel + ball) * 0.5;
            let mid_w = frame.world.transform_point3(mid);
            if !planted && def.plan_steps {
                if lift >= PLAN_EASE_M {
                    st.landing = None;
                } else if let Some((prev_mid, prev_lift)) = st.prev {
                    // Decided while coming down (on the way up a low swing is still leaving its
                    // last tread), and refined each frame as it comes: a landing foot slows to rest,
                    // so it covers about half what its present speed would carry it (a first build
                    // projected the whole speed, chose the tread beyond, and left heels over the
                    // tread below).
                    let falling = (prev_lift - lift) / dt.max(1e-4);
                    if falling > 0.05 {
                        let to_touch = (lift.max(0.0) / falling).min(0.3);
                        let landing_w = mid_w + flat(mid_w - prev_mid) / dt.max(1e-4) * to_touch * 0.5;
                        let step = ask(to_model.transform_point3(landing_w)).and_then(|g| g.step);
                        st.landing = Some(step.map(|s| (s.axis, s.centre, s.depth)));
                    }
                }
            }
            st.prev = Some((mid_w, lift));
            // Worked out in the swing, and once more on the touchdown frame itself from where the foot
            // is then (from the frame before, a foot landed off by its last frame's travel: 0.4 mm into
            // the step above at 30 frames a second where 60 passed).
            if (st.lock.is_none() && (!planted || !st.planted)) || st.plan.is_none() {
                // The FLAT foot's ends, from the heel's sole point along the way the foot points:
                // a foot touches down pitched toes-up and then flattens about its locked heel,
                // growing forward (a first build centred the pitched foot, and a man's 26 cm foot
                // on a 28 cm tread then grew its toe tips past the nosing).
                let fwd = flat(soles_w[1] - soles_w[0]).normalize_or_zero();
                let (back, front) = rig.reach[i];
                let hb = rig.heel_to_ball[i];
                // About whichever end it touches down on (the heel in a walk, the ball in a crouch
                // or a step up).
                let ends = if ball.y < heel.y {
                    [soles_w[1] - fwd * (hb + back), soles_w[1] + fwd * front]
                } else {
                    [soles_w[0] - fwd * back, soles_w[0] + fwd * (hb + front)]
                };
                // Centred on the tread, but never with the end pointing up the stair closer than
                // `tread_margin_m` to the next riser: that end would go into the step above, while
                // the other end may hang over the nosing behind (as people's heels do on a narrow
                // tread). A man's 26 cm foot on a 28 cm tread leaves 1 cm either side, which the
                // projection of a touchdown cannot always hit.
                let margin = def.tread_margin_m;
                st.plan = st.landing.flatten().map(|(axis, centre, depth)| {
                    let (a, b) = (ends[0].dot(axis), ends[1].dot(axis));
                    let mut shift = centre - (a + b) * 0.5;
                    let over = a.max(b) + shift - (centre + depth * 0.5 - margin);
                    if over > 0.0 {
                        shift -= over;
                    }
                    (axis, centre, shift.clamp(-depth * 0.6, depth * 0.6))
                });
            }
            let ease = if planted { 1.0 } else { (1.0 - lift.max(0.0) / PLAN_EASE_M).clamp(0.0, 1.0) };
            let plan_w = st.plan.map_or(Vec3::ZERO, |(axis, _, s)| axis * s);
            if planted && !st.planted {
                if st.lock.is_some() {
                    // Planted again while the last lock is still letting go (a clip's contact can
                    // lift for a frame mid-stance): hold the same spot, never a snap back to the
                    // clip's.
                    st.lock_w = 1.0;
                } else {
                    // A new step: keep this spot (planned onto the tread's middle on a stair, where
                    // the swing has already carried it), held fully from the first frame.
                    st.lock = Some(soles_w[carrier] + plan_w);
                    st.lock_pivot = carrier;
                    st.lock_w = 1.0;
                }
                // The next swing chooses its own landing.
                st.landing = None;
            }
            let on_tread = planted && st.plan.is_some();
            if !planted && st.lock.is_none() {
                // The swing's share of the plan (the lock carries it once planted).
                let p = to_model.transform_vector3(plan_w) * ease;
                st.swing_plan = p;
            } else {
                st.swing_plan = Vec3::ZERO;
            }
            st.planted = planted;
            if let (Some(lock), true) = (st.lock, carrier != st.lock_pivot) {
                // The foot rolled onto its other sole point: lock that one where it now is.
                st.lock = Some(soles_w[carrier] + flat(lock - soles_w[st.lock_pivot]));
                st.lock_pivot = carrier;
            }
            let held = soles_w[st.lock_pivot];
            if let Some(lock) = st.lock {
                if flat(lock - held).length() > def.lock_break_m {
                    st.lock = None;
                    st.lock_w = 0.0;
                } else if planted {
                    st.lock_w = 1.0;
                } else {
                    st.lock_w -= if def.lock_out_s > 1e-4 { dt / def.lock_out_s } else { 1.0 };
                    if st.lock_w <= 0.0 {
                        st.lock = None;
                        st.lock_w = 0.0;
                    }
                }
            }
            let shift_m = match st.lock {
                Some(lock) => to_model.transform_vector3(flat(lock - held)) * st.lock_w,
                None => Vec3::ZERO,
            } + st.swing_plan;
            let sole = sole.map(|p| p + shift_m);
            let (heel, ball, ankle) = (sole[1], sole[2], ankle + shift_m);
            // THE GROUND under the heel and the ball (the back of the heel reads the heel's ground,
            // the toe tips the ball's).
            let gh = ask(heel);
            let gb = ask(ball);
            // THE SLOPE first: a planted foot turns to the surface (about its ankle, so the turn and
            // the lift below cannot fight: a first build turned it about the lowest point AFTER
            // lifting, and a foot already heel-down at a heel strike went a further 11 degrees into
            // a 20% slope, 3.7 cm deep).
            let tilt = match gb.or(gh) {
                Some(hit) => {
                    let probe_at = if gb.is_some() { ball } else { heel };
                    let n_m = slope_normal(&mut ask, probe_at, &hit, &to_model, def);
                    let full = Quat::from_rotation_arc(Vec3::Y, n_m);
                    let (axis, angle) = full.to_axis_angle();
                    if angle > 1e-4 {
                        Quat::from_axis_angle(axis, angle.min(def.max_tilt_deg.to_radians()) * c)
                    } else {
                        Quat::IDENTITY
                    }
                }
                None => Quat::IDENTITY,
            };
            let sole = sole.map(|p| ankle + tilt * (p - ankle));
            // Then the lowest of the four sole points against the ground is the pivot (on a step's
            // edge the ball rests and the heel hangs: the toe pivot).
            // Each point's clearance over the plane of the ground found near it (the back of the
            // heel is a few centimetres behind the heel's sample, the toe tips ahead of the ball's:
            // on a slope, read at the sample's own height they sank 1.7 cm into a 20% slope).
            let clear = |p: Vec3, g: &Option<GroundHit>| {
                g.map(|g| {
                    let gp = to_model.transform_point3(g.point);
                    let n = to_model.transform_vector3(g.normal).normalize_or_zero();
                    let gy = if n.y > 0.2 { gp.y - (n.x * (p.x - gp.x) + n.z * (p.z - gp.z)) / n.y } else { gp.y };
                    p.y - gy
                })
            };
            let hits = [gh, gh, gb, gb];
            let mut best: Option<(Vec3, GroundHit, f32)> = None;
            for k in 0..4 {
                if let (Some(cl), Some(hit)) = (clear(sole[k], &hits[k]), hits[k]) {
                    if best.is_none_or(|b| cl < b.2) {
                        best = Some((sole[k], hit, cl));
                    }
                }
            }
            let Some((_pivot, pivot_hit, mut current)) = best else {
                report.feet[i] = FootReport { contact: c, ground: None, ankle: clip_ankle_w, locked: false, on_tread };
                targets[i] = Some((ankle, model[leg.foot].to_scale_rotation_translation().1));
                continue;
            };
            // A SWINGING foot also asks the ground under its own back of the heel and toe tips (the
            // planes under the heel and the ball miss a riser between them: swinging over a nosing,
            // the toe tips went through the step above, 15 to 16 cm deep in the second anim-stairs
            // rig sweep), and the ground a little ahead of its toes, so the lift rises before the
            // next step (as a foot clears a stair) instead of popping up at the riser. A planted
            // foot keeps the planes (on a slope they read the slope; the planner keeps it off risers).
            let mut aim = current;
            if !planted {
                let own = |p: Vec3, g: Option<GroundHit>| g.map(|g| p.y - to_model.transform_point3(g.point).y);
                // The toe tips are read where the toe cap's front is at the toes' pitch this frame
                // (`toe_ahead_at`), the back of the heel where the heel counter is at the foot's
                // (`heel_behind_at`: walking down a stair it cut into the riser behind), each at its
                // sole point's own height (the cap and the counter stand higher, so this errs clear).
                let fwd_m = Vec3::new(ball.x - heel.x, 0.0, ball.z - heel.z).normalize_or_zero();
                let tip = sole[3] + fwd_m * rig.toe_ahead_at(i, model);
                let back = sole[0] - fwd_m * rig.heel_behind_at(i, model);
                for (p, at) in [(sole[0], back), (sole[3], tip)] {
                    if let Some(cl) = own(p, ask(at)) {
                        current = current.min(cl);
                    }
                }
                aim = current;
                // Only until the foot starts down onto its landing (the planner's `landing`): the
                // ground ahead of a landing foot's toes is the step beyond, and reading it then held
                // the foot a riser over its own tread until it planted.
                if def.plan_steps && st.landing.is_none() {
                    if let Some(cl) = own(sole[3], ask(tip + fwd_m * def.swing_ahead_m)) {
                        aim = aim.min(cl);
                    }
                }
            }
            let want = lift.max(0.0) * (1.0 - c);
            let dy_now = want - aim;
            // A planted foot is put on the ground at once; a swinging one eases, but never through
            // the ground.
            st.lift = if planted { dy_now } else { st.lift + (dy_now - st.lift) * follow(def.lift_follow_s) };
            let dy = st.lift.max(-current);
            dys[i] = dy;
            // The foot turned about its ankle and lifted by the pivot's clearance.
            let ankle_t = ankle + Vec3::Y * dy;
            let foot_rot = (tilt * model[leg.foot].to_scale_rotation_translation().1).normalize();
            targets[i] = Some((ankle_t, foot_rot));
            report.feet[i] = FootReport {
                contact: c,
                ground: Some(pivot_hit.point),
                ankle: frame.world.transform_point3(ankle_t),
                locked: st.lock.is_some() && st.lock_w > 0.5,
                on_tread,
            };
        }
        // THE PELVIS. With the legs solved (Full): lowered by the larger drop, so the lower foot can
        // reach its ground, raised (less far) when both feet stand higher; each leg then reaches its
        // own. With no leg IK (Pelvis): the whole body moves, so it rises until no planted foot is
        // in the ground (the most any planted foot needs), and holds while no foot is planted (a
        // run's flight between strides), easing back only in the air.
        let planted: Vec<f32> = (0..2).filter(|&i| contact(i) > 0.5).map(|i| dys[i]).collect();
        let want = match mode {
            FootMode::Pelvis => planted.iter().copied().reduce(f32::max).unwrap_or(self.pelvis_target),
            _ => dys[0].min(dys[1]),
        };
        let want = want.clamp(-def.max_drop_m / scale, def.max_raise_m / scale);
        let want = if frame.airborne || frame.pelvis_fixed { 0.0 } else { want };
        self.pelvis_target = want;
        if !self.started {
            self.pelvis = want;
            self.started = true;
        } else {
            self.pelvis += (want - self.pelvis) * follow(def.pelvis_follow_s);
        }
        // And never so high that a planted foot is out of its leg's reach (the follow lags a root
        // climbing a stair while the trailing foot stays planted on a tread below): the hip may be
        // no further from each planted ankle's target than the leg can stretch.
        if mode == FootMode::Full && !frame.pelvis_fixed {
            for i in 0..2 {
                let (Some((t, _)), true) = (targets[i], contact(i) > 0.5) else { continue };
                let leg = rig.legs[i];
                let hip = model[leg.upper].w_axis.truncate();
                let knee = model[leg.lower].w_axis.truncate();
                let ankle = model[leg.foot].w_axis.truncate();
                let reach = ((knee - hip).length() + (ankle - knee).length()) * 0.995;
                let d = hip - t;
                let across = d.x * d.x + d.z * d.z;
                if reach * reach > across {
                    self.pelvis = self.pelvis.min(-d.y + (reach * reach - across).sqrt());
                }
            }
            self.pelvis = self.pelvis.max(-def.max_drop_m / scale);
        }
        let shift = Vec3::Y * self.pelvis;
        for m in model.iter_mut() {
            m.w_axis += shift.extend(0.0);
        }
        let hips_parent = skel.parents[rig.hips];
        let parent_inv = match hips_parent {
            Some(p) => model[p].inverse(),
            None => skel.outer[rig.hips].inverse(),
        };
        // The hips' local translation carries the offset, so the pose and the model agree.
        if let Some(j) = pose.joints.get_mut(rig.hips) {
            let hips_model = model[rig.hips].w_axis.truncate();
            j.t = parent_inv.transform_point3(hips_model);
        }
        if mode == FootMode::Full {
            for (i, t) in targets.iter().enumerate() {
                let Some((ankle_t, foot_rot)) = *t else { continue };
                if frame.pelvis_fixed && !frame.seat_plant {
                    solve_seated_leg(skel, pose, model, rig.legs[i], ankle_t, foot_rot);
                } else {
                    solve_leg(skel, pose, model, rig.legs[i], ankle_t, foot_rot);
                }
            }
            crate::retarget::apply_twists(skel, humanoid, pose);
            model_matrices(skel, pose, model);
        }
        report.pelvis_m = self.pelvis * scale;
        report.queries = queries;
        self.report = report;
        report
    }
}

/// The surface's up under a foot's pivot, in model space: the ground's own normal when it gives a
/// sloped one, else read from two more samples beside the pivot. A difference bigger than a slope
/// over the span (a riser) reads as level.
fn slope_normal(ask: &mut dyn FnMut(Vec3) -> Option<GroundHit>, pivot: Vec3, hit: &GroundHit, to_model: &Mat4, def: &FeetDef) -> Vec3 {
    let n = to_model.transform_vector3(hit.normal).normalize_or_zero();
    if n.y > 0.0 && n.y < 0.9999 {
        return n;
    }
    let s = def.slope_probe_m.max(0.01);
    let g0 = to_model.transform_point3(hit.point).y;
    let gx = ask(pivot + Vec3::X * s).map(|g| to_model.transform_point3(g.point).y);
    let gz = ask(pivot + Vec3::Z * s).map(|g| to_model.transform_point3(g.point).y);
    let riser = s * 0.75; // steeper than about 37 degrees is a step, not a slope
    let dx = gx.map(|y| y - g0).filter(|d| d.abs() <= riser).unwrap_or(0.0);
    let dz = gz.map(|y| y - g0).filter(|d| d.abs() <= riser).unwrap_or(0.0);
    Vec3::new(-dx / s, 1.0, -dz / s).normalize()
}

/// A rotation's angle-axis about `axis` by `angle`, or the identity for a zero axis.
fn turn(axis: Vec3, angle: f32) -> Quat {
    let a = axis.normalize_or_zero();
    if a == Vec3::ZERO || !angle.is_finite() {
        Quat::IDENTITY
    } else {
        Quat::from_axis_angle(a, angle)
    }
}

/// THE TWO-BONE ANALYTIC SOLVE, in model space: new model rotations for the upper and lower joints
/// of a chain `a` (hip) -> `b` (knee) -> `c` (ankle) that put `c` on `t`, keeping the bend in the
/// plane the chain already bends in (`pole`, the way a straight chain bends, when it is straight).
/// The triangle's sides are the two bones and the reach (clamped to what the bones can span); the
/// knee's angle is set by the law of cosines with the line from hip to ankle held, then the whole
/// chain is swung about the hip onto the target (the method of D. Holden's "Simple Two Joint IK").
///
/// A nearly straight chain's bend plane says little (the MakeHuman idle's knees, a few degrees
/// bent and splayed with the legs' rest, bowed outward as the solve bent them across a slope in the
/// rig's planet vantage), so first the chain is twisted about the hip-to-ankle line toward `pole`
/// (the knee over the toes, as a leg bends), fully when the chain is straight and not at all once
/// its knee is bent 30 degrees or more (a clip's own deliberate bend, a crouch's knees, is kept).
pub fn two_bone(a: Vec3, b: Vec3, c: Vec3, t: Vec3, ra: Quat, rb: Quat, pole: Vec3) -> (Quat, Quat) {
    let eps = 1e-4;
    let lab = (b - a).length();
    let lcb = (c - b).length();
    if lab < eps || lcb < eps {
        return (ra, rb);
    }
    let (b, ra, rb) = {
        let line = (c - a).normalize_or_zero();
        let perp = |v: Vec3| v - line * v.dot(line);
        let knee = perp(b - a).normalize_or_zero();
        let want = perp(pole).normalize_or_zero();
        let bend = std::f32::consts::PI - (-(b - a).normalize()).dot((c - b).normalize()).clamp(-1.0, 1.0).acos();
        let keep = (bend / 30f32.to_radians()).clamp(0.0, 1.0);
        let aim = if knee == Vec3::ZERO { want } else { (knee * keep + want * (1.0 - keep)).normalize_or_zero() };
        if knee == Vec3::ZERO || aim == Vec3::ZERO || line == Vec3::ZERO || knee.dot(aim) < -0.999 {
            (b, ra, rb)
        } else {
            // Both are square to the line, so this turns about it: the ankle stays where it is.
            let twist = Quat::from_rotation_arc(knee, aim);
            (a + twist * (b - a), (twist * ra).normalize(), (twist * rb).normalize())
        }
    };
    let lat = (t - a).length().clamp((lab - lcb).abs() + eps, lab + lcb - eps);
    let cos = |x: f32| x.clamp(-1.0, 1.0).acos();
    let ac = (c - a).normalize_or_zero();
    let ab = (b - a).normalize_or_zero();
    let ba = -ab;
    let bc = (c - b).normalize_or_zero();
    let ac_ab_0 = cos(ac.dot(ab));
    let ba_bc_0 = cos(ba.dot(bc));
    let ac_ab_1 = cos((lcb * lcb - lab * lab - lat * lat) / (-2.0 * lab * lat));
    let ba_bc_1 = cos((lat * lat - lab * lab - lcb * lcb) / (-2.0 * lab * lcb));
    let mut axis0 = (c - a).cross(b - a);
    if axis0.length_squared() < 1e-10 {
        axis0 = (c - a).cross(pole);
    }
    let axis0 = axis0.normalize_or_zero();
    // Open or close the triangle with the hip-to-ankle line held: the hip turns the thigh, the knee
    // turns the shin, both about the bend axis.
    let r0 = turn(axis0, ac_ab_1 - ac_ab_0);
    let r1 = turn(axis0, ba_bc_1 - ba_bc_0);
    let b1 = a + r0 * (b - a);
    let c1 = a + r0 * (c - a);
    let c2 = b1 + r1 * (c1 - b1);
    // Then swing the whole chain about the hip onto the target.
    let from = (c2 - a).normalize_or_zero();
    let to = (t - a).normalize_or_zero();
    let r2 = if from == Vec3::ZERO || to == Vec3::ZERO { Quat::IDENTITY } else { Quat::from_rotation_arc(from, to) };
    ((r2 * r0 * ra).normalize(), (r2 * r1 * r0 * rb).normalize())
}

/// Bend one leg so its ankle reaches `ankle_t` and its foot ends turned `foot_rot` (model space),
/// writing the local rotations into `pose` and refreshing `model`.
fn solve_leg(skel: &Skeleton, pose: &mut Pose, model: &mut Vec<Mat4>, leg: Leg, ankle_t: Vec3, foot_rot: Quat) {
    let pos = |m: &[Mat4], k: usize| m[k].w_axis.truncate();
    let rot = |m: &[Mat4], k: usize| m[k].to_scale_rotation_translation().1.normalize();
    let (a, b, c) = (pos(model, leg.upper), pos(model, leg.lower), pos(model, leg.foot));
    // The way the knee points as it bends: over the toes (the way the foot points along the ground),
    // the character's front (+Z) if the foot points nowhere.
    let toes = pos(model, leg.toes) - c;
    let over_toes = Vec3::new(toes.x, 0.0, toes.z).try_normalize().unwrap_or(Vec3::Z);
    let (ra, rb) = two_bone(a, b, c, ankle_t, rot(model, leg.upper), rot(model, leg.lower), over_toes);
    set_model_rotation(skel, pose, model, leg.upper, ra);
    set_model_rotation(skel, pose, model, leg.lower, rb);
    set_model_rotation(skel, pose, model, leg.foot, foot_rot);
}

/// A SEATED leg (the hips and thighs on the seat, as the clip lays them): only the lower leg swings,
/// about its knee, so nothing is stretched and the thigh is never driven down through the seat.
/// With the floor within the shin's reach under the knee, the ankle goes onto the floor's height
/// on the circle the shin sweeps there, nearest its target (a low seat: the foot slides forward,
/// the knee bending more), and the foot takes `foot_rot` (flat on the floor). With the floor
/// further down than the shin reaches (a seat high for the body), the shin points toward the target
/// and the foot keeps the clip's turn: the leg hangs from the seat, the foot clear of the floor.
fn solve_seated_leg(skel: &Skeleton, pose: &mut Pose, model: &mut Vec<Mat4>, leg: Leg, ankle_t: Vec3, foot_rot: Quat) {
    let pos = |m: &[Mat4], k: usize| m[k].w_axis.truncate();
    let rot = |m: &[Mat4], k: usize| m[k].to_scale_rotation_translation().1.normalize();
    let (b, c) = (pos(model, leg.lower), pos(model, leg.foot));
    let shin = (c - b).length();
    if shin < 1e-4 {
        return;
    }
    let drop = b.y - ankle_t.y;
    let (want, on_floor) = if drop >= shin {
        // Out of reach: hang toward the target, the whole shin's length from the knee.
        (b + (ankle_t - b).normalize_or_zero() * shin, false)
    } else {
        // Within reach: on the floor, the shin's length from the knee, toward the target across the
        // floor (the way the toes point when the target is under the knee).
        let across = Vec3::new(ankle_t.x - b.x, 0.0, ankle_t.z - b.z);
        let toes = pos(model, leg.toes) - c;
        let way = if across.length() > 0.02 { across.normalize() } else { Vec3::new(toes.x, 0.0, toes.z).try_normalize().unwrap_or(Vec3::Z) };
        let r = (shin * shin - drop.max(0.0) * drop.max(0.0)).sqrt();
        (Vec3::new(b.x, ankle_t.y, b.z) + way * r, true)
    };
    let (from, to) = ((c - b).normalize_or_zero(), (want - b).normalize_or_zero());
    if from != Vec3::ZERO && to != Vec3::ZERO {
        set_model_rotation(skel, pose, model, leg.lower, (Quat::from_rotation_arc(from, to) * rot(model, leg.lower)).normalize());
    }
    if on_floor {
        set_model_rotation(skel, pose, model, leg.foot, foot_rot);
    }
}

/// Give joint `k` the model-space rotation `r` (its local rotation under its parent's posed one),
/// and refresh the model matrices.
pub(crate) fn set_model_rotation(skel: &Skeleton, pose: &mut Pose, model: &mut Vec<Mat4>, k: usize, r: Quat) {
    let parent = match skel.parents[k] {
        Some(p) => model[p].to_scale_rotation_translation().1,
        None => skel.outer[k].to_scale_rotation_translation().1,
    };
    if let Some(j) = pose.joints.get_mut(k) {
        j.r = (parent.normalize().inverse() * r).normalize();
    }
    model_matrices(skel, pose, model);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE TWO-BONE SOLVE REACHES ITS TARGET: a bent leg (hip, knee forward, ankle) is solved onto
    /// targets nearer, further, to the side and lower, and the ankle lands on each within a
    /// millimetre, the bones keep their lengths, and the knee stays on the front side. A target out
    /// of reach straightens the leg toward it.
    /// Seen red 2026-11-09 with the swing applied before the knee's opening (`r0 * r2` for
    /// `r2 * r0`): "target [0.05, 0.25, 0.1]: the ankle lands 0.011 m off".
    #[test]
    fn the_two_bone_solve_reaches_its_target_and_keeps_the_knee_in_front() {
        let a = Vec3::new(0.1, 0.9, 0.0);
        let b = Vec3::new(0.1, 0.5, 0.05);
        let c = Vec3::new(0.1, 0.1, 0.0);
        let (lab, lcb) = ((b - a).length(), (c - b).length());
        for t in [Vec3::new(0.1, 0.25, 0.1), Vec3::new(0.05, 0.25, 0.1), Vec3::new(0.2, 0.3, -0.1), Vec3::new(0.1, 0.15, 0.2), Vec3::new(0.1, 0.4, 0.0)] {
            let (ra, rb) = two_bone(a, b, c, t, Quat::IDENTITY, Quat::IDENTITY, Vec3::Z);
            let b2 = a + ra * (b - a);
            let c2 = b2 + rb * (c - b);
            assert!((c2 - t).length() < 1e-3, "target {t}: the ankle lands {:.3} m off", (c2 - t).length());
            assert!(((b2 - a).length() - lab).abs() < 1e-4 && ((c2 - b2).length() - lcb).abs() < 1e-4, "the bones keep their lengths");
            // The knee bends forward of the hip-ankle line.
            let line = (c2 - a).normalize();
            let off = (b2 - a) - line * (b2 - a).dot(line);
            assert!(off.z > 0.0, "target {t}: the knee bends backward ({off})");
        }
        let far = Vec3::new(0.1, -0.5, 0.0);
        let (ra, rb) = two_bone(a, b, c, far, Quat::IDENTITY, Quat::IDENTITY, Vec3::Z);
        let c2 = a + ra * (b - a) + rb * (c - b);
        assert!((c2 - a).length() > lab + lcb - 0.01, "out of reach the leg straightens");
        // A straight leg bends forward by the pole.
        let straight_c = Vec3::new(0.1, 0.9 - lab - lcb, 0.0);
        let straight_b = Vec3::new(0.1, 0.9 - lab, 0.0);
        let (ra, _) = two_bone(a, straight_b, straight_c, Vec3::new(0.1, 0.3, 0.0), Quat::IDENTITY, Quat::IDENTITY, Vec3::Z);
        assert!((a + ra * (straight_b - a)).z > 0.01, "a straight leg bends its knee forward");
    }

    /// A NEARLY STRAIGHT LEG BENDS ITS KNEE OVER THE TOES: a leg whose knee is bent a few degrees
    /// outward (the MakeHuman idle's, splayed with the legs' rest), solved onto a target 12 cm
    /// higher, brings its knee forward over the toes rather than bowing it further out; a knee the
    /// clip bends 40 degrees sideways (a deliberate pose) keeps its own plane.
    /// Seen red 2026-11-09 with the twist toward the pole taken out (`keep` held at 1): "the knee
    /// bows out: it points 90 degrees from over the toes".
    #[test]
    fn a_nearly_straight_leg_bends_its_knee_over_the_toes() {
        let a = Vec3::new(0.1, 0.9, 0.0);
        let (lab, lcb) = (0.4_f32, 0.4_f32);
        // The knee 3 degrees out to the side (+X) of the straight line.
        let out = 3f32.to_radians();
        let b = a + Vec3::new(out.sin(), -out.cos(), 0.0) * lab;
        let c = b + Vec3::new(-out.sin(), -out.cos(), 0.0) * lcb;
        let t = c + Vec3::new(0.0, 0.12, 0.0);
        let (ra, rb) = two_bone(a, b, c, t, Quat::IDENTITY, Quat::IDENTITY, Vec3::Z);
        let b2 = a + ra * (b - a);
        let c2 = b2 + rb * (c - b);
        assert!((c2 - t).length() < 1e-3, "the ankle lands {:.4} m off", (c2 - t).length());
        let line = (c2 - a).normalize();
        let knee = ((b2 - a) - line * (b2 - a).dot(line)).normalize();
        let off = knee.dot(Vec3::Z).clamp(-1.0, 1.0).acos().to_degrees();
        assert!(off < 20.0, "the knee bows out: it points {off:.0} degrees from over the toes");
        // A knee bent 40 degrees to the side keeps its plane.
        let side = 20f32.to_radians();
        let b = a + Vec3::new(side.sin(), -side.cos(), 0.0) * lab;
        let c = b + Vec3::new(-side.sin(), -side.cos(), 0.0) * lcb;
        let (ra, _) = two_bone(a, b, c, c + Vec3::new(0.0, 0.05, 0.0), Quat::IDENTITY, Quat::IDENTITY, Vec3::Z);
        let b2 = a + ra * (b - a);
        assert!(b2.x - a.x > 0.1 && b2.z.abs() < 0.01, "a deliberate sideways bend is kept: the knee at {b2}");
    }
}

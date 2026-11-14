//! INTERACTIONS ANCHORED TO THINGS (character animation rung 2b, 2026-11-10;
//! docs/design/character-animation.md sections 3.3, 4 "Hands" and "Look-at", and 17).
//!
//! MotionBricks' "Smart Primitives" place target poses around an object so sitting or climbing
//! snaps together; this is the same idea written as our own data, with no network model:
//!
//! - AN ANCHOR is a row of `data/characters/interactions.ron`: where it sits on its object (the
//!   person's root while it holds, and which way they face), where to stand before it (the
//!   approach), the contact targets (hips on the seat, hands on the rungs or the work surface, a hand
//!   on the door's handle), each phase's clip or target pose (enter, hold, exit), whether the root is
//!   driven, and how long each phase may take. OBJECTS NAME THEIR ANCHORS (a machine definition's
//!   `anchors`, a structure type's, an opening style's), so a new chair is a new row, never code.
//! - MOTION WARPING aligns the clip's contact frame with the real anchor: the hips' offset from where
//!   the clip puts them to where THIS seat puts them is eased in over the enter clip up to its
//!   contact frame (the root's travel and turn are eased over the same window by the engine), held
//!   while seated, and eased out over the exit clip until the hips leave the seat.
//! - Where no clip fits (the free tier of Quaternius's library has no lie-down and no ladder climb),
//!   TARGET POSES with analytic IK stand in: lying is the idle turned onto its back on the mattress;
//!   climbing puts each hand on a rung and each foot on a rung below, from the ladder piece's own
//!   rung spacing, with a diagonal gait; a station puts both hands on the work surface; a door one
//!   hand on its handle. Said plainly in the design, section 17.
//! - HANDS: two-bone arm IK (the same analytic solve as the legs, `feet::two_bone`) to a target with
//!   the palm and fingers turned as asked, and a small GRIP library (`data/characters/grips.ron`:
//!   relaxed, power, pinch, staff, two-handed, rung, flat) blended per finger.
//! - LOOK-AT: the neck, upper neck and head turn toward a point, then the eyes take what is left,
//!   each within its limits, at a person's turn rate (`humanity_core::turning::LOOK`).
//! - FIRST PERSON: the head and neck (and everything under them) are folded to the base of the neck
//!   for the eye camera (`collapse_subtree`), the standard way a first-person body hides its head
//!   (a hidden bone scaled to nothing), so the eye sees its arms, legs and clothes and never the inside
//!   of its own head.
//!
//! GAMEPLAY NEVER WAITS FOR ANIMATION. Nothing here moves a body or decides anything: the engine's
//! gameplay grants an interaction (sitting down, climbing a ladder, using a station), and this only
//! shows it. The capsule, reach, move checks and footing stay the truth.

use std::collections::BTreeMap;

use glam::{Mat3, Mat4, Quat, Vec3};
use serde::Deserialize;

use crate::clip::ClipSet;
use crate::feet::{set_model_rotation, two_bone};
use crate::pose::{model_matrices, Pose};
use crate::{HumanoidDef, Skeleton};

// ── The data ─────────────────────────────────────────────────────────────

/// What an anchor shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum AnchorKind {
    /// Seated, the hips on a seat (chairs, benches, a bed's edge).
    Sit,
    /// Lying on the back (beds).
    Lie,
    /// On a ladder, hands and feet on its rungs.
    Climb,
    /// Standing at a station or workbench, the hands on its work surface.
    Station,
    /// A hand on a door's handle while it opens.
    Door,
}

impl AnchorKind {
    /// The stance the network carries for it (humanity_core::stance codes): sitting and lying are
    /// Sit, a ladder Climb, the rest standing.
    pub fn stance_code(self) -> u8 {
        match self {
            AnchorKind::Sit | AnchorKind::Lie => 2,
            AnchorKind::Climb => 3,
            AnchorKind::Station | AnchorKind::Door => 0,
        }
    }
}

/// A part of the body an anchor puts on its object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ContactPart {
    /// The hips (seated: the buttocks and thighs on the seat; lying: the back and hips on the
    /// mattress).
    Hips,
    LeftHand,
    RightHand,
    /// Both hands (a ladder's rungs, a work surface): placed either side of the point.
    Hands,
    /// Both feet (a ladder's rungs below the hands).
    Feet,
}

/// One contact target: a part of the body and where it goes on the object.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ContactDef {
    pub part: ContactPart,
    /// Where, in the object's frame: metres, +Y up, the object's own +Z, from the middle of its
    /// footprint on the floor (a placed machine's origin).
    pub at: [f32; 3],
    /// The hand's grip (`grips.ron`), for a hand contact.
    #[serde(default)]
    pub grip: Option<String>,
}

/// A target pose built with IK where no clip fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum TargetPose {
    /// The idle turned onto its back, its hips on the mattress.
    Lie,
    /// Hands and feet on the ladder's rungs, from the piece's own spacing.
    Climb,
    /// The idle with the hands reaching their contacts (a work surface, a handle).
    Reach,
}

/// One phase of an anchor: a clip (`clips.ron`), or a target pose, and the longest it may take.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PhaseDef {
    #[serde(default)]
    pub clip: Option<String>,
    #[serde(default)]
    pub pose: Option<TargetPose>,
    /// Seconds: a clip plays out within it (cut short past it); a target pose blends in over it.
    pub max_s: f32,
}

fn d_reach() -> f32 {
    1.5
}

fn d_tuck() -> bool {
    true
}

/// One anchor: a row of `data/characters/interactions.ron`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AnchorDef {
    pub id: String,
    pub kind: AnchorKind,
    /// Where the person's root (between the feet, on the floor) stands while it holds, in the
    /// object's frame. For lying, the point on the floor under the hips.
    pub root: [f32; 3],
    /// Which way they face while it holds: a turn about up from the object's +Z, degrees (a chair
    /// facing its -Z: 180). For lying, the way the feet point (the head lies the other way).
    pub facing_deg: f32,
    /// Where they stand before it, in the object's frame.
    pub approach: [f32; 3],
    pub contacts: Vec<ContactDef>,
    pub enter: PhaseDef,
    pub hold: PhaseDef,
    pub exit: PhaseDef,
    /// The root (the capsule) is moved by the interaction, inside the grant (sitting down, lying
    /// down); false for one that follows the capsule as the existing systems move it (the ladder,
    /// a station, a door).
    pub root_driven: bool,
    /// The farthest the person may stand from the approach point to be granted it, metres.
    #[serde(default = "d_reach")]
    pub reach_m: f32,
    /// A station's: how far either way along the object's x the hands may slide to stay in front
    /// of the person (the length of a workbench they can work along).
    #[serde(default)]
    pub span_m: Option<f32>,
    /// A seat's: how far it slides back along its own +Z (away from the way it faces) when the
    /// person sits at it while it is tucked in, its approach crowded by the desk or the table it
    /// stands under, metres (the game's `engine::interact::plan_seat`). 0, the default: it is never
    /// moved (a couch).
    #[serde(default)]
    pub pull_out_m: f32,
    /// A seat's: pushed back in where it stood once the person is up (true, the default), or left
    /// where it was pulled out to (false).
    #[serde(default = "d_tuck")]
    pub tuck_after: bool,
}

impl AnchorDef {
    /// The contact for a part, if it has one.
    pub fn contact(&self, part: ContactPart) -> Option<&ContactDef> {
        self.contacts.iter().find(|c| c.part == part)
    }

    /// A phase's definition.
    pub fn phase(&self, p: Phase) -> &PhaseDef {
        match p {
            Phase::Enter => &self.enter,
            Phase::Hold => &self.hold,
            Phase::Exit => &self.exit,
        }
    }

    /// A point of the object's frame in the frame of the person standing at the anchor's root
    /// facing its way (their model space before their own scale): what the remote end needs to put
    /// the hips on THIS seat when all it knows is where the person stands and the anchor's id.
    pub fn in_person_frame(&self, at: [f32; 3]) -> Vec3 {
        let d = Vec3::from(at) - Vec3::from(self.root);
        Quat::from_rotation_y(-self.facing_deg.to_radians()) * d
    }
}

/// `data/characters/interactions.ron`: every anchor, read and checked against the clips and grips.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Anchors {
    pub anchors: Vec<AnchorDef>,
}

impl Anchors {
    pub fn parse(text: &str) -> Result<Self, String> {
        let anchors: Vec<AnchorDef> = crate::parse_ron(text, "data/characters/interactions.ron")?;
        Ok(Anchors { anchors })
    }

    /// Refuse a row that cannot play: an id listed twice, a phase with neither a clip nor a pose
    /// (or one naming a clip clips.ron lacks), a phase that may take no time, a grip grips.ron
    /// lacks, a sit with no hips contact.
    pub fn check(&self, clips: &ClipSet, grips: &Grips) -> Result<(), String> {
        for (i, a) in self.anchors.iter().enumerate() {
            let at = format!("data/characters/interactions.ron: anchor {}", a.id);
            if a.id.is_empty() || a.id.len() > humanity_core::stance::ANCHOR_ID_MAX {
                return Err(format!("{at}: an id must be 1 to {} characters", humanity_core::stance::ANCHOR_ID_MAX));
            }
            if self.anchors[..i].iter().any(|b| b.id == a.id) {
                return Err(format!("{at} is listed twice"));
            }
            for (name, p) in [("enter", &a.enter), ("hold", &a.hold), ("exit", &a.exit)] {
                if p.clip.is_none() && p.pose.is_none() {
                    return Err(format!("{at}: its {name} phase has neither a clip nor a target pose"));
                }
                if let Some(c) = &p.clip {
                    if clips.index(c).is_none() {
                        return Err(format!("{at}: its {name} phase plays clip {c}, which is not in data/characters/clips.ron"));
                    }
                }
                if !(p.max_s.is_finite() && p.max_s > 0.0) {
                    return Err(format!("{at}: its {name} phase may take no time (max_s {})", p.max_s));
                }
            }
            for c in &a.contacts {
                if let Some(g) = &c.grip {
                    if grips.index(g).is_none() {
                        return Err(format!("{at}: grip {g} is not in data/characters/grips.ron"));
                    }
                }
            }
            if matches!(a.kind, AnchorKind::Sit | AnchorKind::Lie) && a.contact(ContactPart::Hips).is_none() {
                return Err(format!("{at}: a {:?} anchor needs a Hips contact", a.kind));
            }
        }
        Ok(())
    }

    pub fn index(&self, id: &str) -> Option<usize> {
        self.anchors.iter().position(|a| a.id == id)
    }
}

// ── The grips ────────────────────────────────────────────────────────────

/// The five fingers, in the order a grip names them.
pub const FINGERS: [&str; 5] = ["thumb", "index", "middle", "ring", "little"];

/// The three joints of each finger, by role suffix (VRM 1.0: the thumb's are Metacarpal, Proximal,
/// Distal; the others Proximal, Intermediate, Distal).
const FINGER_SEGMENTS: [[&str; 3]; 5] = [
    ["ThumbMetacarpal", "ThumbProximal", "ThumbDistal"],
    ["IndexProximal", "IndexIntermediate", "IndexDistal"],
    ["MiddleProximal", "MiddleIntermediate", "MiddleDistal"],
    ["RingProximal", "RingIntermediate", "RingDistal"],
    ["LittleProximal", "LittleIntermediate", "LittleDistal"],
];

/// One grip: how far each finger's three joints curl toward the palm, degrees. A finger the grip
/// does not name keeps the clip's own pose (a pinch moves the thumb and index and leaves the rest).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct GripDef {
    pub id: String,
    pub fingers: BTreeMap<String, [f32; 3]>,
    /// A grip that takes both hands on one shaft (two-handed, a staff): how far along the shaft from
    /// the main hand the second hand closes, metres.
    #[serde(default)]
    pub second_hand_m: Option<f32>,
}

/// `data/characters/grips.ron`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Grips {
    pub grips: Vec<GripDef>,
}

impl Grips {
    pub fn parse(text: &str) -> Result<Self, String> {
        let grips: Vec<GripDef> = crate::parse_ron(text, "data/characters/grips.ron")?;
        for (i, g) in grips.iter().enumerate() {
            if grips[..i].iter().any(|o| o.id == g.id) {
                return Err(format!("data/characters/grips.ron: grip {} is listed twice", g.id));
            }
            for f in g.fingers.keys() {
                if !FINGERS.contains(&f.as_str()) {
                    return Err(format!("data/characters/grips.ron: grip {} names finger {f}, which is not one of {FINGERS:?}", g.id));
                }
            }
        }
        Ok(Grips { grips })
    }

    pub fn index(&self, id: &str) -> Option<usize> {
        self.grips.iter().position(|g| g.id == id)
    }
}

// ── The hands' rig ───────────────────────────────────────────────────────

/// One arm's joints by role and its hand's frame at rest.
#[derive(Debug, Clone, PartialEq)]
pub struct Arm {
    pub upper: usize,
    pub lower: usize,
    pub hand: usize,
    /// The hand's palm normal (out of the palm) and its fingers' way (wrist to the middle finger's
    /// root) at rest, model space.
    pub palm_rest: Vec3,
    pub along_rest: Vec3,
    /// From the wrist joint to the middle of the palm at rest, model space: where a hand target is
    /// measured to (a grip closes around the palm, not the wrist).
    pub palm_offset: Vec3,
    /// Each finger's three joints (None for a rig without that finger), and each joint's curl axis
    /// in its parent's frame at rest.
    pub fingers: [Option<[(usize, Vec3); 3]>; 5],
    /// How far the hand's skin reaches out of the palm past `palm_offset`'s point, metres, measured
    /// from the body's hand vertices at rest (`HandRig::with_palm_depth`): the thumb's ball and the
    /// heel of the hand stand that far proud of it, so a palm laid on a surface puts that point on it.
    pub palm_depth: f32,
}

/// A skeleton's arms and hands (left, right).
#[derive(Debug, Clone, PartialEq)]
pub struct HandRig {
    pub arms: [Arm; 2],
    /// The neck, upper neck and head (look-at), and the eyes.
    pub neck: Option<usize>,
    pub upper_neck: Option<usize>,
    pub head: Option<usize>,
    pub eyes: [Option<usize>; 2],
    /// The hips (lying turns the body about them).
    pub hips: usize,
    /// Every leg joint (the upper and lower legs, the feet, the toes, both sides): lying lays them
    /// straight from the rest pose.
    pub legs: Vec<usize>,
    /// Each shoulder (the upper arm's joint) at rest, model space, and how far each arm reaches
    /// from it to the palm's middle (the bones' rest lengths and the palm offset).
    pub shoulders: [Vec3; 2],
    pub reach: [f32; 2],
}

impl HandRig {
    /// The arms of a skeleton; None when an arm joint has no bone.
    pub fn new(skel: &Skeleton, humanoid: &HumanoidDef) -> Option<Self> {
        let j = |r: &str| skel.joint_by_role(humanoid, r);
        let rest_pos = |k: usize| skel.rest_position(k);
        let arm = |side: &str, left: bool| -> Option<Arm> {
            let upper = j(&format!("{side}UpperArm"))?;
            let lower = j(&format!("{side}LowerArm"))?;
            let hand = j(&format!("{side}Hand"))?;
            let wrist = rest_pos(hand);
            let middle = j(&format!("{side}MiddleProximal")).map(rest_pos);
            let index = j(&format!("{side}IndexProximal")).map(rest_pos);
            let little = j(&format!("{side}LittleProximal")).map(rest_pos);
            // The way the hand points: to the middle finger's root, else along the forearm.
            let along = middle.map(|m| m - wrist).unwrap_or(wrist - rest_pos(lower)).normalize_or_zero();
            // Across the palm toward the thumb side, and the palm's normal from the two (the left
            // hand's palm faces along x its own way: along x across for the left, across x along
            // for the right; checked on a T-posed hand, palm down, thumb forward).
            let across = match (index, little) {
                (Some(i), Some(l)) => (i - l).normalize_or_zero(),
                _ => Vec3::Z,
            };
            let mut palm = if left { along.cross(across) } else { across.cross(along) }.normalize_or_zero();
            if palm == Vec3::ZERO {
                palm = -Vec3::Y;
            }
            let palm_offset = middle.map(|m| (m - wrist) * 0.55 + palm * 0.012).unwrap_or(along * 0.05);
            let mut fingers: [Option<[(usize, Vec3); 3]>; 5] = [None, None, None, None, None];
            for (f, segs) in FINGER_SEGMENTS.iter().enumerate() {
                let ids: Option<Vec<usize>> = segs.iter().map(|s| j(&format!("{side}{s}"))).collect();
                let Some(ids) = ids else { continue };
                let mut out = [(0usize, Vec3::X); 3];
                for (k, &id) in ids.iter().enumerate() {
                    // The segment's own way: to the next joint, else its parent's way on.
                    let next = ids.get(k + 1).map(|&n| rest_pos(n)).unwrap_or_else(|| rest_pos(id) + (rest_pos(id) - skel.parents[id].map(rest_pos).unwrap_or(wrist)));
                    let d = (next - rest_pos(id)).normalize_or_zero();
                    // Curling turns the segment toward the palm's side: about d x palm.
                    let axis_model = d.cross(palm).normalize_or_zero();
                    let parent_rot = skel.parents[id].map(|p| skel.rest_world_rotation(p)).unwrap_or(Quat::IDENTITY);
                    let axis_local = (parent_rot.inverse() * axis_model).normalize_or_zero();
                    out[k] = (id, if axis_local == Vec3::ZERO { Vec3::X } else { axis_local });
                }
                fingers[f] = Some(out);
            }
            Some(Arm { upper, lower, hand, palm_rest: palm, along_rest: along, palm_offset, fingers, palm_depth: 0.0 })
        };
        let arms = [arm("left", true)?, arm("right", false)?];
        let shoulders = [rest_pos(arms[0].upper), rest_pos(arms[1].upper)];
        let reach = [0, 1].map(|s| {
            let a = &arms[s];
            (rest_pos(a.lower) - rest_pos(a.upper)).length() + (rest_pos(a.hand) - rest_pos(a.lower)).length() + a.palm_offset.length()
        });
        Some(HandRig {
            arms,
            neck: j("neck"),
            upper_neck: j("upperNeck"),
            head: j("head"),
            eyes: [j("leftEye"), j("rightEye")],
            hips: j("hips")?,
            legs: ["UpperLeg", "LowerLeg", "Foot", "Toes"].iter().flat_map(|r| ["left", "right"].map(|s| j(&format!("{s}{r}")))).flatten().collect(),
            shoulders,
            reach,
        })
    }
}

impl HandRig {
    /// This rig with each hand's palm-side depth measured from its vertices at rest (model space,
    /// left then right): the most any of them stands out of the palm past the palm's point.
    pub fn with_palm_depth(mut self, hands: [&[Vec3]; 2]) -> Self {
        for (side, verts) in hands.iter().enumerate() {
            let arm = &mut self.arms[side];
            let depth = verts.iter().map(|v| v.dot(arm.palm_rest)).fold(f32::NEG_INFINITY, f32::max);
            if depth.is_finite() {
                arm.palm_depth = depth.max(0.0);
            }
        }
        self
    }
}

/// Where a hand goes (model space): the palm's middle on `at`, the palm facing `palm`, the
/// fingers pointing `fingers`, closed in grip `grip` (an index into `Grips`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HandTarget {
    pub at: Vec3,
    pub palm: Vec3,
    pub fingers: Vec3,
    pub grip: Option<usize>,
}

/// The hands' and feet's targets for one frame (model space), and how much of each to take.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Limbs {
    pub hands: [Option<HandTarget>; 2],
    /// Each ankle's target (the climb's feet on their rungs).
    pub feet: [Option<Vec3>; 2],
    /// 0 leaves the pose as it is, 1 puts each limb on its target.
    pub weight: f32,
}

/// The rotation taking the rest frame (`along`, `palm`) onto the wanted one (`fingers`, `palm`),
/// both made square first.
fn frame_turn(along: Vec3, palm: Vec3, want_along: Vec3, want_palm: Vec3) -> Quat {
    let basis = |a: Vec3, p: Vec3| -> Option<Mat3> {
        let a = a.normalize_or_zero();
        let side = a.cross(p).normalize_or_zero();
        if a == Vec3::ZERO || side == Vec3::ZERO {
            return None;
        }
        let p = side.cross(a);
        Some(Mat3::from_cols(a, p, side))
    };
    match (basis(along, palm), basis(want_along, want_palm)) {
        (Some(from), Some(to)) => Quat::from_mat3(&(to * from.transpose())).normalize(),
        _ => Quat::IDENTITY,
    }
}

/// Put the hands (two-bone arm IK, the hand turned as asked, the fingers closed in the grip) and
/// the feet (two-bone leg IK) on their targets, by `limbs.weight`. `model` is refreshed.
pub fn apply_limbs(rig: &HandRig, legs: Option<&crate::FootRig>, skel: &Skeleton, humanoid: &HumanoidDef, grips: &Grips, pose: &mut Pose, model: &mut Vec<Mat4>, limbs: &Limbs) {
    let w = limbs.weight.clamp(0.0, 1.0);
    if w <= 0.0 || model.len() < skel.joint_count() {
        return;
    }
    let pos = |m: &[Mat4], k: usize| m[k].w_axis.truncate();
    let rot = |m: &[Mat4], k: usize| m[k].to_scale_rotation_translation().1.normalize();
    for (side, target) in limbs.hands.iter().enumerate() {
        let Some(t) = target else { continue };
        let arm = &rig.arms[side];
        let rest_hand = skel.rest_world_rotation(arm.hand);
        // The hand's wanted turn, and where the wrist must be for the palm's middle to land on the
        // target with the hand turned so.
        let turn = frame_turn(arm.along_rest, arm.palm_rest, t.fingers, t.palm);
        let hand_rot = (turn * rest_hand).normalize();
        let wrist_t = t.at - turn * arm.palm_offset;
        let (a, b, c) = (pos(model, arm.upper), pos(model, arm.lower), pos(model, arm.hand));
        let wrist_want = c.lerp(wrist_t, w);
        // The elbow bends down and out, away from the body's middle, as elbows do.
        let out = if side == 0 { Vec3::X } else { -Vec3::X };
        let pole = (-Vec3::Y * 0.8 + out * 0.5 - Vec3::Z * 0.2).normalize();
        let (ra, rb) = two_bone(a, b, c, wrist_want, rot(model, arm.upper), rot(model, arm.lower), pole);
        set_model_rotation(skel, pose, model, arm.upper, ra);
        set_model_rotation(skel, pose, model, arm.lower, rb);
        let now = rot(model, arm.hand);
        set_model_rotation(skel, pose, model, arm.hand, crate::pose::nlerp(now, hand_rot, w));
        if let Some(g) = t.grip.and_then(|g| grips.grips.get(g)) {
            apply_grip(arm, skel, pose, g, w);
        }
    }
    if let Some(legs) = legs {
        for (side, target) in limbs.feet.iter().enumerate() {
            let Some(t) = target else { continue };
            let leg = legs.legs[side];
            let (a, b, c) = (pos(model, leg.upper), pos(model, leg.lower), pos(model, leg.foot));
            let toes = pos(model, leg.toes) - c;
            let over_toes = Vec3::new(toes.x, 0.0, toes.z).try_normalize().unwrap_or(Vec3::Z);
            let (ra, rb) = two_bone(a, b, c, c.lerp(*t, w), rot(model, leg.upper), rot(model, leg.lower), over_toes);
            set_model_rotation(skel, pose, model, leg.upper, ra);
            set_model_rotation(skel, pose, model, leg.lower, rb);
        }
    }
    crate::retarget::apply_twists(skel, humanoid, pose);
    model_matrices(skel, pose, model);
}

/// Both hands on one shaft carried in front (a two-handed tool, a staff; grips.ron's
/// `second_hand_m`): the main (right) hand before its hip, the second hand `second_m` metres along the
/// shaft, which runs up, across and a little forward, the palms toward it. Model space, from the
/// hips' height in `model`. The prop itself is not drawn yet (no item has a model): the hands hold
/// where it would be.
pub fn two_handed_limbs(rig: &HandRig, model: &[Mat4], grip: Option<usize>, second_m: f32) -> Limbs {
    let hips_y = model.get(rig.hips).map_or(0.95, |m| m.w_axis.y);
    let main = Vec3::new(-0.13, hips_y + 0.02, 0.28);
    let shaft = Vec3::new(0.55, 0.75, 0.35).normalize();
    let second = main + shaft * second_m;
    let mut l = Limbs { weight: 1.0, ..Default::default() };
    l.hands[1] = Some(HandTarget { at: main, palm: Vec3::X, fingers: Vec3::Z, grip });
    l.hands[0] = Some(HandTarget { at: second, palm: -Vec3::X, fingers: Vec3::Z, grip });
    l
}

/// Close one hand's fingers toward a grip, per finger: each named finger's joints turned about
/// their curl axes from the rest by the grip's angles, blended from the pose's own by `w`.
pub fn apply_grip(arm: &Arm, skel: &Skeleton, pose: &mut Pose, grip: &GripDef, w: f32) {
    for (f, name) in FINGERS.iter().enumerate() {
        let (Some(segs), Some(angles)) = (arm.fingers[f], grip.fingers.get(*name)) else { continue };
        for (k, &(joint, axis)) in segs.iter().enumerate() {
            let Some(j) = pose.joints.get_mut(joint) else { continue };
            let want = (Quat::from_axis_angle(axis, angles[k].to_radians()) * skel.rest[joint].r).normalize();
            j.r = crate::pose::nlerp(j.r, want, w);
        }
    }
}

// ── Look-at ──────────────────────────────────────────────────────────────

/// How far the head (neck, upper neck and head together) and then the eyes turn to look, degrees.
pub const HEAD_YAW_MAX_DEG: f32 = 70.0;
pub const HEAD_PITCH_UP_DEG: f32 = 40.0;
pub const HEAD_PITCH_DOWN_DEG: f32 = 55.0;
pub const EYE_YAW_MAX_DEG: f32 = 25.0;
pub const EYE_PITCH_MAX_DEG: f32 = 20.0;
/// The shares of the head's turn the neck, upper neck and head take.
const LOOK_SHARES: [f32; 3] = [0.3, 0.3, 0.4];

/// A person's look from frame to frame: the head's yaw and pitch, each turning the way a person
/// turns (`turning::LOOK`). State, like a position.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LookState {
    pub yaw: humanity_core::turning::Turn,
    pub pitch: humanity_core::turning::Turn,
    pub started: bool,
}

/// The head's wanted yaw and pitch (radians, +yaw to the left, +pitch up) toward a direction in
/// model space, clamped to the head's limits, and what is left over for the eyes (clamped to
/// theirs).
pub fn look_angles(dir: Vec3) -> ((f32, f32), (f32, f32)) {
    let d = dir.normalize_or_zero();
    if d == Vec3::ZERO {
        return ((0.0, 0.0), (0.0, 0.0));
    }
    let yaw = d.x.atan2(d.z);
    let pitch = d.y.clamp(-1.0, 1.0).asin();
    let hy = yaw.clamp(-HEAD_YAW_MAX_DEG.to_radians(), HEAD_YAW_MAX_DEG.to_radians());
    let hp = pitch.clamp(-HEAD_PITCH_DOWN_DEG.to_radians(), HEAD_PITCH_UP_DEG.to_radians());
    let ey = (yaw - hy).clamp(-EYE_YAW_MAX_DEG.to_radians(), EYE_YAW_MAX_DEG.to_radians());
    let ep = (pitch - hp).clamp(-EYE_PITCH_MAX_DEG.to_radians(), EYE_PITCH_MAX_DEG.to_radians());
    ((hy, hp), (ey, ep))
}

/// Turn the head toward `target` (model space; None looks ahead) at a person's turn rate, the
/// neck, upper neck and head each taking a share, then the eyes the rest. `weight` 0 leaves the
/// pose as it is. Returns the head's yaw and pitch now (radians).
pub fn look_at(rig: &HandRig, skel: &Skeleton, pose: &mut Pose, model: &mut Vec<Mat4>, state: &mut LookState, target: Option<Vec3>, weight: f32, dt: f32) -> (f32, f32) {
    let Some(head) = rig.head else { return (0.0, 0.0) };
    if model.len() < skel.joint_count() {
        return (0.0, 0.0);
    }
    let eye_at = model[head].w_axis.truncate() + Vec3::new(0.0, 0.08, 0.08);
    let ((hy, hp), (ey, ep)) = match target {
        Some(t) => look_angles(t - eye_at),
        None => ((0.0, 0.0), (0.0, 0.0)),
    };
    let w = weight.clamp(0.0, 1.0);
    let (want_y, want_p) = (hy * w, hp * w);
    if !state.started {
        state.yaw = humanity_core::turning::Turn { angle: want_y, rate: 0.0 };
        state.pitch = humanity_core::turning::Turn { angle: want_p, rate: 0.0 };
        state.started = true;
    } else {
        state.yaw.toward(want_y, dt, humanity_core::turning::LOOK);
        state.pitch.toward(want_p, dt, humanity_core::turning::LOOK);
    }
    let (yaw, pitch) = (state.yaw.angle, state.pitch.angle);
    let rot = |m: &[Mat4], k: usize| m[k].to_scale_rotation_translation().1.normalize();
    for (k, joint) in [rig.neck, rig.upper_neck, Some(head)].into_iter().enumerate() {
        let Some(j) = joint else { continue };
        let s = LOOK_SHARES[k];
        let delta = Quat::from_rotation_y(yaw * s) * Quat::from_rotation_x(-pitch * s);
        let r = rot(model, j);
        set_model_rotation(skel, pose, model, j, (delta * r).normalize());
    }
    // The eyes take the rest (only when the head is near where it is going: a head mid-turn leads
    // with the eyes a little, which the leftover already is).
    let (ey, ep) = (ey * w, ep * w);
    for eye in rig.eyes.into_iter().flatten() {
        let delta = Quat::from_rotation_y(yaw + ey) * Quat::from_rotation_x(-(pitch + ep));
        let parent_free = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-pitch);
        let r = rot(model, eye);
        set_model_rotation(skel, pose, model, eye, (delta * parent_free.inverse() * r).normalize());
    }
    (yaw, pitch)
}

// ── First person ─────────────────────────────────────────────────────────

/// Fold every joint from `root` down (the neck: the neck, the head, the eyes and the jaw) to a
/// point at `root`'s position, scaled to nothing: their vertices are drawn as a point at the base of
/// the neck, the standard way a first-person body hides its head (a bone scaled to zero), so the eye
/// camera sees the arms, legs and clothes and never the inside of the head. Only the palette the
/// first-person view draws is folded; nothing else reads it.
pub fn collapse_subtree(skel: &Skeleton, model: &mut [Mat4], root: usize) {
    if root >= model.len() {
        return;
    }
    let at = model[root].w_axis.truncate();
    let point = Mat4::from_scale_rotation_translation(Vec3::splat(0.0), Quat::IDENTITY, at);
    let mut inside = vec![false; skel.joint_count()];
    for &j in &skel.order {
        inside[j] = j == root || skel.parents[j].is_some_and(|p| inside[p]);
        if inside[j] && j < model.len() {
            model[j] = point;
        }
    }
}

// ── Phases, weights and warping ──────────────────────────────────────────

/// Where an engaged person is in their anchor. The wire codes (`code`) never change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Enter,
    Hold,
    Exit,
}

impl Phase {
    pub fn code(self) -> u8 {
        match self {
            Phase::Enter => 0,
            Phase::Hold => 1,
            Phase::Exit => 2,
        }
    }

    pub fn from_code(c: u8) -> Option<Self> {
        Some(match c {
            0 => Phase::Enter,
            1 => Phase::Hold,
            2 => Phase::Exit,
            _ => return None,
        })
    }
}

/// How long a phase lasts on a body: its clip's length within `max_s`, or `max_s` for a target
/// pose. A looped hold lasts until the person leaves (this returns its clip's length).
pub fn phase_len(def: &AnchorDef, phase: Phase, clips: &ClipSet) -> f32 {
    let p = def.phase(phase);
    match p.clip.as_deref().and_then(|c| clips.index(c)) {
        Some(i) => clips.get(i).duration.min(p.max_s).max(1e-3),
        None => p.max_s.max(1e-3),
    }
}

/// How far into the interaction's look a person is: `engage` blends the locomotion pose into the
/// interaction's (in over the start of enter, out over the end of exit), `warp` how much of the
/// contact correction is on (in up to the enter clip's contact frame, out from the exit clip's
/// lift-off).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weights {
    pub engage: f32,
    pub warp: f32,
}

/// The seconds of blending into and out of an interaction from walking.
pub const ENGAGE_BLEND_S: f32 = 0.25;

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The weights at `t` seconds into `phase`. `contact_s` is when the enter clip's hips reach the
/// seat (`contact_time`), `release_s` when the exit clip's leave it.
pub fn weights(def: &AnchorDef, phase: Phase, t: f32, clips: &ClipSet, contact_s: f32, release_s: f32) -> Weights {
    let len = phase_len(def, phase, clips);
    // A clip carries its own way in and out (a sit down from standing): it is blended in from the
    // walk briefly. A target pose has no motion of its own, so it is blended over its whole phase.
    let has_clip = def.phase(phase).clip.is_some();
    let blend = if has_clip { ENGAGE_BLEND_S.min(len * 0.4).max(1e-3) } else { len };
    match phase {
        Phase::Enter => Weights { engage: smooth(t / blend), warp: smooth(t / contact_s.clamp(1e-3, len)) },
        Phase::Hold => Weights { engage: 1.0, warp: 1.0 },
        Phase::Exit => Weights { engage: smooth((len - t) / blend), warp: 1.0 - smooth(t / release_s.clamp(1e-3, len)) },
    }
}

/// The hips joint's model position through a clip on a skeleton, frame by frame (the hips are a
/// root joint, so no other joint is needed).
fn hips_track(clips: &ClipSet, clip: usize, skel: &Skeleton, hips: usize) -> Vec<Vec3> {
    let c = clips.get(clip);
    let outer = skel.outer[hips];
    let rest = skel.rest[hips].t;
    match c.translations.get(hips).and_then(|t| t.as_ref()) {
        Some(track) => track.iter().map(|d| outer.transform_point3(rest + *d)).collect(),
        None => vec![outer.transform_point3(rest); c.frames.max(1)],
    }
}

/// Where the hold clip puts the hips while seated (its first frame), model space.
pub fn seated_hips(def: &AnchorDef, clips: &ClipSet, skel: &Skeleton, hips: usize) -> Option<Vec3> {
    let hold = def.hold.clip.as_deref().and_then(|c| clips.index(c))?;
    hips_track(clips, hold, skel, hips).first().copied()
}

/// When the enter clip's hips come within 1 cm of the seated hips (its contact frame), seconds;
/// the phase's length when it has no clip or never does.
pub fn contact_time(def: &AnchorDef, clips: &ClipSet, skel: &Skeleton, hips: usize) -> f32 {
    let len = phase_len(def, Phase::Enter, clips);
    let (Some(enter), Some(seated)) = (def.enter.clip.as_deref().and_then(|c| clips.index(c)), seated_hips(def, clips, skel, hips)) else { return len };
    let c = clips.get(enter);
    hips_track(clips, enter, skel, hips)
        .iter()
        .position(|p| (p.y - seated.y).abs() < 0.01)
        .map_or(len, |f| (f as f32 / c.fps.max(1.0)).clamp(0.05, len))
}

/// When the exit clip's hips rise 2 cm off the seat (its lift-off), seconds.
pub fn release_time(def: &AnchorDef, clips: &ClipSet, skel: &Skeleton, hips: usize) -> f32 {
    let len = phase_len(def, Phase::Exit, clips);
    let (Some(exit), Some(seated)) = (def.exit.clip.as_deref().and_then(|c| clips.index(c)), seated_hips(def, clips, skel, hips)) else { return len };
    let c = clips.get(exit);
    hips_track(clips, exit, skel, hips)
        .iter()
        .position(|p| p.y > seated.y + 0.02)
        .map_or(len, |f| (f as f32 / c.fps.max(1.0)).clamp(0.05, len))
}

/// What one engaged person needs to be posed this frame (model space: the person's own frame at
/// their root, before their scale).
#[derive(Debug, Clone, Copy)]
pub struct Engagement<'a> {
    pub def: &'a AnchorDef,
    pub phase: Phase,
    pub t: f32,
    /// Where the hips joint goes while it holds (seated: over the seat by the flesh under the
    /// hips, `contact drop`; lying: over the mattress by the back's), model space; None when the
    /// anchor has no hips contact.
    pub hips_target: Option<Vec3>,
    /// The contact correction on (false is the red twin's switch: the clips play as made).
    pub warp: bool,
    pub contact_s: f32,
    pub release_s: f32,
}

/// The interaction's pose: the phase's clip (or target pose), blended from the locomotion pose by
/// the engage weight, with the hips warped onto their target. `locomotion` is the graph's pose this
/// frame; `out` receives the result (twist joints redone). Returns the weights.
#[allow(clippy::too_many_arguments)]
pub fn pose(e: &Engagement, clips: &ClipSet, skel: &Skeleton, humanoid: &HumanoidDef, rig: &HandRig, idle: Option<usize>, locomotion: &Pose, out: &mut Pose, scratch: &mut Pose) -> Weights {
    let w = weights(e.def, e.phase, e.t, clips, e.contact_s, e.release_s);
    let p = e.def.phase(e.phase);
    // The phase's own pose.
    let clip = p.clip.as_deref().and_then(|c| clips.index(c));
    match (clip, p.pose) {
        (Some(c), _) => {
            let t = match e.phase {
                Phase::Hold => e.t,
                _ => e.t.min(phase_len(e.def, e.phase, clips)),
            };
            clips.get(c).sample(skel, t, scratch);
        }
        (None, Some(target)) => {
            // Target poses start from the idle (or the locomotion pose when there is none).
            match idle {
                Some(i) => clips.get(i).sample(skel, e.t, scratch),
                None => {
                    scratch.joints.clear();
                    scratch.joints.extend_from_slice(&locomotion.joints);
                }
            }
            if target == TargetPose::Lie {
                lie_down(skel, rig, e, scratch);
            }
        }
        (None, None) => {
            scratch.joints.clear();
            scratch.joints.extend_from_slice(&locomotion.joints);
        }
    }
    // The contact correction for a seat: the hips moved from where the hold clip puts them to the
    // target, eased in up to the contact frame and out from the lift-off (motion warping).
    if e.warp && e.def.kind == AnchorKind::Sit {
        if let (Some(target), Some(seated)) = (e.hips_target, seated_hips(e.def, clips, skel, rig.hips)) {
            let offset = (target - seated) * w.warp;
            let outer_inv = match skel.parents[rig.hips] {
                Some(_) => Mat4::IDENTITY,
                None => skel.outer[rig.hips].inverse(),
            };
            if let Some(j) = scratch.joints.get_mut(rig.hips) {
                j.t += outer_inv.transform_vector3(offset);
            }
        }
    }
    crate::pose::blend_into(locomotion, scratch, w.engage, out);
    crate::retarget::apply_twists(skel, humanoid, out);
    w
}

/// The lying target pose: the body turned about its hips onto its back (its front up, its head the
/// opposite way to its facing, the feet toward it), the hips on their target (the mattress under
/// the back, by the engagement's `hips_target`); eased by the engage weight's own blend outside.
fn lie_down(skel: &Skeleton, rig: &HandRig, e: &Engagement, pose: &mut Pose) {
    let hips = rig.hips;
    if pose.joints.get(hips).is_none() {
        return;
    }
    let outer = skel.outer[hips];
    let (_, outer_rot, _) = outer.to_scale_rotation_translation();
    // From the REST pelvis, not the idle's: the idle tips its pelvis, and turned onto its back with
    // the legs straight from it the ankles stood 22.6 cm over the covers (the second rig sweep).
    let model_rot = (outer_rot * skel.rest[hips].r).normalize();
    // Onto the back: up turns to -Z (the head away from the facing), front turns to +Y.
    let lie = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let new_rot = (lie * model_rot).normalize();
    let target = e.hips_target.unwrap_or(Vec3::new(0.0, 0.5, 0.0));
    let out = pose.joints.get_mut(hips).expect("read above");
    out.r = (outer_rot.inverse() * new_rot).normalize();
    out.t = outer.inverse().transform_point3(target);
    // The legs lie straight and together (the rest pose's), not in the idle's stance: turned onto
    // its back, the idle's forward foot rose toward the ceiling (the first rig sweep).
    for &j in &rig.legs {
        if let (Some(p), Some(r)) = (pose.joints.get_mut(j), skel.rest.get(j)) {
            p.r = r.r;
        }
    }
}

/// The lying pose's hips height over the mattress for a body: the back's thickness under the hips
/// joint once turned onto its back (the distance from the hips joint to the body's back), from the
/// rest skeleton: the hips joint stands this far in front of the back, about the pelvis's depth
/// behind the joint.
pub fn default_back_drop() -> f32 {
    0.11
}

/// The climb's input, in the person's model space (they face the ladder, its rungs running across
/// their x): the line of the rungs ahead (`z`), the ladder's middle across (`x`), the first rung's
/// height and the spacing (from the ladder piece's own size and rung count), how many rungs, and
/// half the rung's length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClimbInput {
    pub z: f32,
    pub x: f32,
    pub first_rung_y: f32,
    pub spacing: f32,
    pub rungs: usize,
    pub half_width: f32,
    /// The rung's thickness (its half): the hand closes around it, the foot stands on its top.
    pub rung_half_m: f32,
    /// How high over the root the hands grip: the person's own shoulders' height plus a little
    /// (`climb_reach`), so a short body's hands are never asked past its reach.
    pub reach_over_root: f32,
    /// Each hand's palm-side depth (`Arm::palm_depth`, left then right): the palm's point stands
    /// that far off the rung's near face, so the palm's skin, not its bones, meets the rung.
    pub palm_depth: [f32; 2],
}

/// How far over their shoulders a climbing person's hands reach at the top of their cycle (metres):
/// each hand takes a rung up to this much above the shoulder and moves on when its rung is two rungs
/// (0.6 m) lower, so a hand works between its chest and over its head, within the shortest body's
/// reach from 30 cm off the rungs (the anchor's root).
const CLIMB_REACH_OVER_SHOULDER: f32 = 0.36;

/// How high over the root a climbing person's hands grip, from their shoulders in this pose
/// (`model`, model space): the higher shoulder's height plus `CLIMB_REACH_OVER_SHOULDER`.
pub fn climb_reach(rig: &HandRig, model: &[Mat4]) -> f32 {
    let y = |k: usize| model.get(k).map_or(1.35, |m| m.w_axis.y);
    y(rig.arms[0].upper).max(y(rig.arms[1].upper)) + CLIMB_REACH_OVER_SHOULDER
}
/// How far the palm's skin stands off a rung's near face (metres): the grip's fingers close round the
/// rung and would reach into its square edges with the palm right on it.
const RUNG_PALM_GAP_M: f32 = 0.01;
/// The share of each limb's two-rung cycle it spends moving to the next rung.
const CLIMB_SWING: f32 = 0.35;

/// The climb's hands and feet on rungs for a root `height` over the ladder's foot (model y 0 is the
/// root), a diagonal gait: each hand and the opposite foot move together, one rung pair at a time,
/// as the root rises (and back down as it falls). Hands grip in grip `grip`, palms toward the
/// ladder, fingers up.
pub fn climb_limbs(c: &ClimbInput, height: f32, grip: Option<usize>, legs: Option<&crate::FootRig>) -> Limbs {
    let rung_y = |i: f32| c.first_rung_y + i * c.spacing - height;
    let last = c.rungs.saturating_sub(1) as f32;
    let s = c.spacing.max(0.05);
    // Each limb's place in its two-rung cycle: the hands reach over the root, the feet stand at it.
    let limb = |reach_over_root: f32, parity: f32| -> Vec3 {
        let r = ((height + reach_over_root - c.first_rung_y) / s - parity) / 2.0;
        let base = r.floor();
        let frac = r - base;
        let from = (base * 2.0 + parity).clamp(0.0, last);
        let to = (base * 2.0 + 2.0 + parity).clamp(0.0, last);
        let k = smooth((frac - (1.0 - CLIMB_SWING)) / CLIMB_SWING);
        let y = rung_y(from) + (rung_y(to) - rung_y(from)) * k;
        // A limb between rungs swings a little away from the ladder.
        let off = (k * std::f32::consts::PI).sin() * 0.06;
        Vec3::new(0.0, y, c.z - off)
    };
    let hand_y = |side: usize| limb(c.reach_over_root, side as f32);
    // Past the top of the ladder (out through a hatch), a hand held on the top rung would be pulled
    // down to the chest and then the belly, under its own cycle's lowest grip (where, on the way up,
    // it would have moved on to the next rung): it lets go there instead (the clip's hand), as a
    // climber's hands leave the rungs for the hatch's rim. Seen in the climb test run to 2.05 m:
    // hands 2.5 to 10.5 cm off a top rung pressed to the chest and the waist, the arm folded tight.
    let let_go = |side: usize, y: f32| {
        let unclamped = ((height + c.reach_over_root - c.first_rung_y) / s - side as f32) / 2.0;
        // Its next rung is past the top (the cycle is clamped), and its rung is under its lowest grip.
        unclamped.floor() * 2.0 + 2.0 + side as f32 > last && y < c.reach_over_root - 2.0 * s * (1.0 - CLIMB_SWING) - 0.02
    };
    let mut out = Limbs { weight: 1.0, ..Default::default() };
    for side in 0..2 {
        let across = if side == 0 { 1.0 } else { -1.0 };
        let p = hand_y(side);
        if let_go(side, p.y) {
            continue;
        }
        out.hands[side] = Some(HandTarget {
            // The palm's point off the rung's near face by the palm's own depth: the skin meets the
            // rung and the grip closes the fingers over it.
            at: Vec3::new(c.x + across * (c.half_width * 0.55).min(0.2), p.y, p.z - c.rung_half_m - c.palm_depth[side] - RUNG_PALM_GAP_M),
            palm: Vec3::Z,
            fingers: Vec3::Y,
            grip,
        });
    }
    if let Some(legs) = legs {
        for side in 0..2 {
            let across = if side == 0 { 1.0 } else { -1.0 };
            // The opposite foot moves with each hand: the left foot shares the right hand's parity.
            let p = limb(0.0, (1 - side) as f32);
            // The ankle over the ball of a foot standing on the rung's top: the ball on the rung,
            // the ankle behind and above it by the foot's own rest.
            let ankle_over_ball = legs.heel_h[side].max(0.05);
            out.feet[side] = Some(Vec3::new(c.x + across * 0.11, p.y + c.rung_half_m + ankle_over_ball, p.z - 0.12));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE CLIMB'S HANDS AND FEET ARE ON RUNGS: at every height up a 10-rung ladder each hand that
    /// is not between rungs is exactly at a rung's height (a hand moves for a share of its cycle,
    /// never both at once), the hands alternate, and the feet stay a rung pair below them; climbing
    /// past the top keeps them on the top rungs until a top rung would be under the hand's own
    /// cycle, where it lets go (out through the hatch).
    /// Seen red 2026-11-10 with both hands on parity 0 (moving together): "both hands between rungs
    /// at once at 0.20 m"; and with the let-go removed: "past the top both hands let go: [true, true]"
    /// (each hand still held, pressed to the waist).
    #[test]
    fn the_climbs_hands_and_feet_are_on_rungs_and_alternate() {
        let c = ClimbInput { z: 0.4, x: 0.0, first_rung_y: 0.15, spacing: 0.3, rungs: 10, half_width: 0.3, rung_half_m: 0.04, reach_over_root: 1.55, palm_depth: [0.02, 0.02] };
        let on_rung = |y_model: f32, height: f32| {
            let world = y_model + height;
            let i = ((world - c.first_rung_y) / c.spacing).round();
            ((world - (c.first_rung_y + i * c.spacing)).abs() < 1e-3, i)
        };
        let mut h = 0.0;
        while h <= 3.0 {
            let l = climb_limbs(&c, h, None, None);
            if l.hands.iter().any(Option::is_none) {
                // A hand let go past the top: never below the height the top rung leaves the reach.
                assert!(h + c.reach_over_root > c.first_rung_y + 9.0 * c.spacing, "a hand let go at {h:.2} m, under the top");
                h += 0.05;
                continue;
            }
            let hands: Vec<(bool, f32)> = l.hands.iter().map(|t| on_rung(t.unwrap().at.y, h)).collect();
            assert!(hands[0].0 || hands[1].0, "both hands between rungs at once at {h:.2} m");
            for (k, (on, i)) in hands.iter().enumerate() {
                if *on {
                    assert!(*i >= 0.0 && *i <= 9.0, "hand {k} on rung {i} at {h:.2} m");
                }
            }
            h += 0.05;
        }
        // Just past the top, both hands hold the top two rungs; well past it, both have let go.
        let l = climb_limbs(&c, 1.6, None, None);
        let ys: Vec<f32> = l.hands.iter().map(|t| t.expect("held just past the top").at.y + 1.6).collect();
        assert!(ys.iter().all(|y| (*y - 2.85).abs() < 0.31), "just past the top the hands hold the top rungs: {ys:?}");
        let l = climb_limbs(&c, 4.0, None, None);
        assert!(l.hands.iter().all(Option::is_none), "past the top both hands let go: {:?}", l.hands.iter().map(Option::is_some).collect::<Vec<_>>());
    }

    /// LOOK-AT KEEPS TO THE HEAD'S LIMITS AND GIVES THE REST TO THE EYES: a point straight ahead
    /// asks nothing; one 90 degrees to the left turns the head 70 and the eyes 20; one far below
    /// turns the head down 55 and the eyes 20 more.
    /// Seen red 2026-11-10 with the head's yaw unclamped: "the head turned 90.0 degrees".
    #[test]
    fn look_at_keeps_to_the_heads_limits_and_gives_the_rest_to_the_eyes() {
        let ((y, p), (ey, ep)) = look_angles(Vec3::Z);
        assert!(y.abs() < 1e-6 && p.abs() < 1e-6 && ey.abs() < 1e-6 && ep.abs() < 1e-6);
        let ((y, _), (ey, _)) = look_angles(Vec3::X);
        assert!((y.to_degrees() - 70.0).abs() < 1e-3, "the head turned {:.1} degrees", y.to_degrees());
        assert!((ey.to_degrees() - 20.0).abs() < 1e-3, "the eyes turned {:.1}", ey.to_degrees());
        let ((_, p), (_, ep)) = look_angles(Vec3::new(0.0, -1.0, 0.2));
        assert!((p.to_degrees() + 55.0).abs() < 1e-3 && (ep.to_degrees() + 20.0).abs() < 1e-3, "down: head {:.1}, eyes {:.1}", p.to_degrees(), ep.to_degrees());
    }

    /// THE PHASE WEIGHTS: entering blends a target pose in over its phase (a clip over its first quarter
    /// second) and
    /// warps in up to the contact frame; holding is all of both; leaving warps out from the
    /// lift-off and blends back to walking over its last quarter second.
    /// Seen red 2026-11-10 with the exit's engage weight run forward (`t / blend`): "leaving, the
    /// interaction is still 1.00 at its end".
    #[test]
    fn the_phase_weights_blend_and_warp_in_and_out() {
        let def = AnchorDef {
            id: "t".into(),
            kind: AnchorKind::Sit,
            root: [0.0; 3],
            facing_deg: 0.0,
            approach: [0.0; 3],
            contacts: vec![],
            enter: PhaseDef { clip: None, pose: Some(TargetPose::Reach), max_s: 1.2 },
            hold: PhaseDef { clip: None, pose: Some(TargetPose::Reach), max_s: 1.0 },
            exit: PhaseDef { clip: None, pose: Some(TargetPose::Reach), max_s: 1.0 },
            root_driven: true,
            reach_m: 1.5,
            span_m: None,
            pull_out_m: 0.0,
            tuck_after: true,
        };
        let clips = ClipSet::default();
        let w0 = weights(&def, Phase::Enter, 0.0, &clips, 0.8, 0.4);
        assert!(w0.engage < 1e-6 && w0.warp < 1e-6);
        // A target pose blends in over its whole phase (1.2 s here).
        let w = weights(&def, Phase::Enter, 0.3, &clips, 0.8, 0.4);
        assert!(w.engage > 0.05 && w.engage < 0.5 && w.warp > 0.1 && w.warp < 0.9, "{w:?}");
        assert!((weights(&def, Phase::Enter, 1.2, &clips, 0.8, 0.4).engage - 1.0).abs() < 1e-6);
        assert_eq!(weights(&def, Phase::Enter, 0.8, &clips, 0.8, 0.4).warp, 1.0);
        assert_eq!(weights(&def, Phase::Hold, 0.4, &clips, 0.8, 0.4), Weights { engage: 1.0, warp: 1.0 });
        let end = weights(&def, Phase::Exit, 1.0, &clips, 0.8, 0.4);
        assert!(end.engage < 1e-6, "leaving, the interaction is still {:.2} at its end", end.engage);
        assert!(end.warp < 1e-6);
        assert!(weights(&def, Phase::Exit, 0.2, &clips, 0.8, 0.4).warp > 0.0);
    }

    /// THE PHASE CODES never change: 0 enter, 1 hold, 2 exit, anything else none.
    #[test]
    fn the_phase_codes_round_trip() {
        for p in [Phase::Enter, Phase::Hold, Phase::Exit] {
            assert_eq!(Phase::from_code(p.code()), Some(p));
        }
        assert_eq!(Phase::from_code(3), None);
    }
}

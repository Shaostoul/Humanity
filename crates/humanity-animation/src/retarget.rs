//! Retargeting by role: a clip made on one rig, played on another body (plan section 2.1: "Every
//! motion source is retargeted by role at load: rest poses aligned, the root's travel scaled by the
//! ratio of leg lengths, in memory only").
//!
//! THE METHOD, for each frame. On the source rig, each mapped joint's turn away from its rest, in
//! model space: `D = W_anim * W_rest^-1`. On the body, the joint playing the same role takes the
//! same turn from ITS rest: `W'_anim = D * C * W'_rest`, and its local rotation is that under its
//! parent's animated orientation. Working in model space, not in each bone's own frame, is what
//! makes two rigs whose bones point their local axes different ways (Blender's roll, Mixamo's
//! conventions, an "Armature" node with Mixamo's 0.01 scale) agree: a turn of the whole forearm is
//! the same turn whatever axis the file called "along the bone".
//!
//! `C` aligns rest poses that differ: for a limb role the standard marks `align`, the shortest turn
//! from the body's rest bone direction to the source's (the bone from this joint to its `aim`
//! joint). For two rigs in the same rest pose it is the identity; for a T-pose clip on an A-pose
//! body it lifts the body's arm onto the clip's before the clip's turns apply, so the arms go where
//! the clip put them rather than 45 degrees off.
//!
//! Joints the body has and the source does not map keep their rest under their parent; joints the
//! source maps and the body lacks are ignored. The hips' travel (the bob and sway of a gait) is the
//! source's, scaled by the ratio of the two rigs' hips heights.

use glam::{Mat4, Quat, Vec3};

use crate::clip::{Clip, SourceClip};
use crate::pose::{model_matrices, Pose};
use crate::{HumanoidDef, Skeleton};

/// Bake `src` (made on `src_skel`) onto `dst` at `fps` frames a second.
pub fn retarget(
    src: &SourceClip,
    src_skel: &Skeleton,
    dst: &Skeleton,
    humanoid: &HumanoidDef,
    fps: f32,
    looped: bool,
    speed_mps: f32,
) -> Clip {
    let fps = if fps.is_finite() && fps > 0.0 { fps } else { 30.0 };
    let frames = if src.duration > 0.0 { ((src.duration * fps).round() as usize).max(1) + 1 } else { 1 };
    let n = dst.joint_count();

    // Which body joint takes its turn from which source joint.
    let source_of: Vec<Option<usize>> = (0..n).map(|k| dst.roles[k].and_then(|r| src_skel.joint(r))).collect();
    let corrections = rest_alignment(src_skel, dst, humanoid);
    let src_rest_world: Vec<Quat> = (0..src_skel.joint_count()).map(|j| src_skel.rest_world_rotation(j)).collect();
    let dst_rest_world: Vec<Quat> = (0..n).map(|k| dst.rest_world_rotation(k)).collect();
    let hips = humanoid.role_index("hips");
    let (src_hips, dst_hips) = (hips.and_then(|r| src_skel.joint(r)), hips.and_then(|r| dst.joint(r)));
    let travel_scale = dst.hips_height(humanoid) / src_skel.hips_height(humanoid);

    let mut rotations: Vec<Option<Vec<Quat>>> = source_of.iter().map(|s| s.map(|_| Vec::with_capacity(frames))).collect();
    let mut translations: Vec<Option<Vec<Vec3>>> = vec![None; n];
    if let (Some(_), Some(dh)) = (src_hips, dst_hips) {
        translations[dh] = Some(Vec::with_capacity(frames));
    }

    let mut src_pose = Pose::default();
    let mut src_model: Vec<Mat4> = Vec::new();
    let mut dst_model = vec![Mat4::IDENTITY; n];
    for f in 0..frames {
        let t = if frames > 1 { (f as f32 / fps).min(src.duration) } else { 0.0 };
        src.sample(src_skel, t, &mut src_pose);
        model_matrices(src_skel, &src_pose, &mut src_model);
        for &k in &dst.order {
            let parent_model = match dst.parents[k] {
                Some(p) => dst_model[p],
                None => dst.outer[k],
            };
            let parent_world = rotation_of(&parent_model);
            let world = match source_of[k] {
                Some(js) => {
                    let d = rotation_of(&src_model[js]) * src_rest_world[js].inverse();
                    (d * corrections[k] * dst_rest_world[k]).normalize()
                }
                None => (parent_world * dst.rest[k].r).normalize(),
            };
            let local_r = (parent_world.inverse() * world).normalize();
            let mut local_t = dst.rest[k].t;
            if Some(k) == dst_hips {
                if let Some(sh) = src_hips {
                    let moved = src_model[sh].w_axis.truncate() - src_skel.rest_position(sh);
                    let want = dst.rest_position(k) + moved * travel_scale;
                    local_t = parent_model.inverse().transform_point3(want);
                    if let Some(tr) = &mut translations[k] {
                        // An offset from the rest (`Clip`), so a reshaped body plays it too.
                        tr.push(local_t - dst.rest[k].t);
                    }
                }
            }
            if let Some(r) = &mut rotations[k] {
                r.push(local_r);
            }
            dst_model[k] = parent_model * Mat4::from_scale_rotation_translation(dst.rest[k].s, local_r, local_t);
        }
    }
    // The clip's gait covers ground in proportion to the legs that walk it: the same turns on legs
    // 6% shorter stride 6% shorter, so the speed it covers on this body is the source's scaled like
    // the hips' travel (2026-11-09: left unscaled, the library's walk played on MakeHuman's female
    // body, whose hips stand 0.83 m to the library's 0.88, slid its planted foot 6%).
    let speed_mps = speed_mps * travel_scale;
    Clip { name: src.name.clone(), duration: src.duration, fps, frames, looped, speed_mps, rotations, translations }
}

/// The rest alignment `C` of each body joint: for a role marked `align` whose joint and aim joint
/// both rigs have, the shortest turn of the body's rest bone onto the source's; for any other joint,
/// its parent's, so a finger rides its hand's alignment (without it, a T-pose clip's finger turns
/// laid on an A-pose hand's rest point every finger 45 degrees off the hand: 2026-11-09, the first
/// clips with fingers); the identity at the root.
pub fn rest_alignment(src: &Skeleton, dst: &Skeleton, humanoid: &HumanoidDef) -> Vec<Quat> {
    let own = |k: usize| -> Option<Quat> {
        let r = dst.roles[k]?;
        let def = &humanoid.roles[r];
        if !def.align {
            return None;
        }
        let aim = def.aim.as_deref().and_then(|a| humanoid.role_index(a))?;
        let (js, sa, ka) = (src.joint(r)?, src.joint(aim)?, dst.joint(aim)?);
        let ds = (src.rest_position(sa) - src.rest_position(js)).normalize_or_zero();
        let dd = (dst.rest_position(ka) - dst.rest_position(k)).normalize_or_zero();
        if ds == Vec3::ZERO || dd == Vec3::ZERO {
            return None;
        }
        Some(Quat::from_rotation_arc(dd, ds))
    };
    let mut c = vec![Quat::IDENTITY; dst.joint_count()];
    for &k in &dst.order {
        c[k] = own(k).unwrap_or_else(|| dst.parents[k].map_or(Quat::IDENTITY, |p| c[p]));
    }
    c
}

/// Give each twist joint its share of its driver's twist about the driver's bone (plan 2.1's twist
/// joints, so a turned wrist twists the forearm's skin along its length instead of collapsing it at
/// the wrist). Runs on a finished local pose. The twist joint must hang from the same parent as its
/// driver (the usual arrangement: a forearm twist joint beside the hand, both under the forearm);
/// one that does not is left at rest.
pub fn apply_twists(skel: &Skeleton, humanoid: &HumanoidDef, pose: &mut Pose) {
    for (r, def) in humanoid.roles.iter().enumerate() {
        let Some(tw) = &def.twist else { continue };
        let Some(k) = skel.joint(r) else { continue };
        let Some(o) = humanoid.role_index(&tw.of).and_then(|or| skel.joint(or)) else { continue };
        if skel.parents[k] != skel.parents[o] || k >= pose.joints.len() || o >= pose.joints.len() {
            continue;
        }
        let axis = skel.rest[o].t.normalize_or_zero();
        if axis == Vec3::ZERO {
            continue;
        }
        let delta = (pose.joints[o].r * skel.rest[o].r.inverse()).normalize();
        let twist = twist_about(delta, axis);
        pose.joints[k].r = (Quat::IDENTITY.slerp(twist, tw.share) * skel.rest[k].r).normalize();
    }
}

/// The twist part of a rotation about `axis` (unit): the swing-twist split.
pub fn twist_about(q: Quat, axis: Vec3) -> Quat {
    let along = axis * Vec3::new(q.x, q.y, q.z).dot(axis);
    let t = Quat::from_xyzw(along.x, along.y, along.z, q.w);
    let len = t.length();
    if len < 1e-6 {
        Quat::IDENTITY
    } else {
        t / len
    }
}

/// The rotation of a matrix that may carry scale (Mixamo's 0.01 armature).
fn rotation_of(m: &Mat4) -> Quat {
    let (_, r, _) = m.to_scale_rotation_translation();
    r.normalize()
}

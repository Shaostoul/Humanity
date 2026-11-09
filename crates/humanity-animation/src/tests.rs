//! The crate's tests: headless, no GPU, no files but the shipped data. Each was seen red first;
//! the red run is named at each test.

use glam::{Mat4, Quat, Vec3};
use humanity_core::skin_data::{AnimChannelData, AnimClipData, ChannelPath, Interpolation};

use crate::clip::{Clip, ClipSet, SourceClip};
use crate::fixtures::{body_rig, foreign_rest_rotation, foreign_rig, humanoid};
use crate::graph::{bracket, stride_rate, Graph, LocomotionInput, StrideDef};
use crate::pose::{apply_rows, model_matrices, palette_rows, Pose};
use crate::retarget::{apply_twists, retarget};
use crate::tiers::{assign, DrawTier, Tiers};
use crate::{Animator, MotionTracker, RoleMap, Skeleton};

fn angle_deg(a: Vec3, b: Vec3) -> f32 {
    a.normalize().dot(b.normalize()).clamp(-1.0, 1.0).acos().to_degrees()
}

/// The shipped standard uses VRM 1.0's names, and the roles VRM 1.0 requires are the ones it marks
/// required (hips, spine, head, and both upper and lower legs, feet, upper and lower arms, hands).
/// Seen red 2026-11-08 with `leftHand` left unmarked in humanoid.ron.
#[test]
fn the_shipped_humanoid_standard_requires_exactly_vrm_1_0s_required_bones() {
    let h = humanoid();
    let mut required: Vec<&str> = h.roles.iter().filter(|r| r.required).map(|r| r.role.as_str()).collect();
    required.sort();
    let mut vrm = vec![
        "hips", "spine", "head", "leftUpperLeg", "leftLowerLeg", "leftFoot", "rightUpperLeg", "rightLowerLeg", "rightFoot",
        "leftUpperArm", "leftLowerArm", "leftHand", "rightUpperArm", "rightLowerArm", "rightHand",
    ];
    vrm.sort();
    assert_eq!(required, vrm);
    // Every finger of VRM 1.0 is a role (the thumb's three bones are Metacarpal, Proximal, Distal).
    for side in ["left", "right"] {
        for f in ["ThumbMetacarpal", "ThumbProximal", "ThumbDistal", "IndexProximal", "IndexIntermediate", "IndexDistal", "LittleDistal"] {
            assert!(h.role_index(&format!("{side}{f}")).is_some(), "{side}{f} missing");
        }
    }
}

/// A rig maps by role, and a role map that hangs a limb off the wrong parent is refused.
/// Seen red 2026-11-08 with the hierarchy check in `Skeleton::from_skin` taken out (the swapped
/// map built a skeleton).
#[test]
fn a_rig_maps_by_role_and_a_map_that_crosses_limbs_is_refused() {
    let h = humanoid();
    let (joints, map) = body_rig();
    let skel = Skeleton::from_skin(&joints, &map, &h).expect("the body rig maps");
    let hand = skel.joint_by_role(&h, "leftHand").expect("left hand");
    assert_eq!(skel.names[hand], "body_leftHand");
    assert!(skel.order.iter().position(|&j| j == skel.parents[hand].unwrap()).unwrap() < skel.order.iter().position(|&j| j == hand).unwrap());
    // Swap the left hand's and the right foot's roles.
    let mut swapped = map.clone();
    swapped.bones.insert("body_leftHand".into(), "rightFoot".into());
    swapped.bones.insert("body_rightFoot".into(), "leftHand".into());
    let err = Skeleton::from_skin(&joints, &swapped, &h).expect_err("a crossed map is refused");
    assert!(err.contains("hangs under"), "{err}");
    // A role the standard does not have is refused when the map is read.
    let text = "(id: \"bad\", bones: { \"x\": \"leftWing\" })";
    assert!(RoleMap::parse(text, &h).is_err());
}

/// At rest every joint's skinning matrix is the identity (model matrix times inverse bind), and
/// a joint turned moves the vertices bound to it by exactly that turn about the joint.
/// Seen red 2026-11-08 with `palette_rows` writing the model matrix without the inverse bind.
#[test]
fn the_palette_is_identity_at_rest_and_moves_a_vertex_with_its_joint() {
    let h = humanoid();
    let (joints, map) = foreign_rig();
    let skel = Skeleton::from_skin(&joints, &map, &h).unwrap();
    let mut model = Vec::new();
    let mut rows = Vec::new();
    model_matrices(&skel, &skel.rest_pose(), &mut model);
    palette_rows(&skel, &model, &mut rows);
    assert_eq!(rows.len(), skel.joint_count() * 3);
    let p = Vec3::new(0.3, 1.2, -0.1);
    for j in 0..skel.joint_count() {
        let r = [rows[j * 3], rows[j * 3 + 1], rows[j * 3 + 2]];
        assert!((apply_rows(&r, p) - p).length() < 1e-4, "joint {} moves a vertex at rest", skel.names[j]);
    }
    // Turn the left forearm 90 degrees about +Y (world) at the elbow.
    let elbow = skel.joint_by_role(&h, "leftLowerArm").unwrap();
    let mut pose = skel.rest_pose();
    let parent_world = skel.rest_world_rotation(skel.parents[elbow].unwrap());
    let turn = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    pose.joints[elbow].r = parent_world.inverse() * turn * skel.rest_world_rotation(elbow);
    model_matrices(&skel, &pose, &mut model);
    rows.clear();
    palette_rows(&skel, &model, &mut rows);
    let e = skel.rest_position(elbow);
    let v = e + Vec3::new(0.1, -0.1, 0.0);
    let r = [rows[elbow * 3], rows[elbow * 3 + 1], rows[elbow * 3 + 2]];
    let want = e + turn * (v - e);
    assert!((apply_rows(&r, v) - want).length() < 1e-4, "{} vs {want}", apply_rows(&r, v));
}

/// A RESHAPED SKELETON (2026-11-09, a body's shape targets moving its joints): the joint moved stands
/// where it was moved, its children where they were (their rest translations worked out again), and
/// skinning at rest is still the identity everywhere (its inverse binds made again), so the reshaped
/// mesh stands as it was shaped.
/// Seen red 2026-11-09 with `reshaped` keeping the old inverse binds: "joint body_leftLowerArm moves a
/// vertex at rest by 0.100".
#[test]
fn a_reshaped_skeleton_moves_its_joints_and_still_skins_to_identity_at_rest() {
    let h = humanoid();
    let (joints, map) = body_rig();
    let skel = Skeleton::from_skin(&joints, &map, &h).unwrap();
    let elbow = skel.joint_by_role(&h, "leftLowerArm").unwrap();
    let hand = skel.joint_by_role(&h, "leftHand").unwrap();
    let mut moves = vec![Vec3::ZERO; skel.joint_count()];
    moves[elbow] = Vec3::new(0.1, 0.0, 0.0);
    let shaped = skel.reshaped(&moves);
    assert!((shaped.rest_position(elbow) - (skel.rest_position(elbow) + Vec3::new(0.1, 0.0, 0.0))).length() < 1e-5);
    assert!((shaped.rest_position(hand) - skel.rest_position(hand)).length() < 1e-5, "an unmoved child stays where it was");
    let mut model = Vec::new();
    let mut rows = Vec::new();
    model_matrices(&shaped, &shaped.rest_pose(), &mut model);
    palette_rows(&shaped, &model, &mut rows);
    let p = Vec3::new(0.3, 1.2, -0.1);
    for j in 0..shaped.joint_count() {
        let r = [rows[j * 3], rows[j * 3 + 1], rows[j * 3 + 2]];
        let moved = (apply_rows(&r, p) - p).length();
        assert!(moved < 1e-4, "joint {} moves a vertex at rest by {moved:.3}", shaped.names[j]);
    }
}

/// glTF's three interpolations: Linear halfway, Step holding, and CubicSpline with zero tangents
/// easing (the Hermite curve's 0.5 at the middle, 0.15625 at a quarter).
/// Seen red 2026-11-08 with CubicSpline read as Linear (0.25 at a quarter, not 0.15625).
#[test]
fn clip_keys_interpolate_as_gltf_says() {
    let h = humanoid();
    let (joints, map) = body_rig();
    let skel = Skeleton::from_skin(&joints, &map, &h).unwrap();
    let hips = skel.joint_by_role(&h, "hips").unwrap();
    let chan = |interp, values: Vec<f32>| AnimChannelData { node: "body_hips".into(), path: ChannelPath::Translation, interpolation: interp, times: vec![0.0, 1.0], values };
    let sample = |c: AnimChannelData, t: f32| {
        let clip = SourceClip::from_data(&AnimClipData { name: "c".into(), channels: vec![c] }, &skel).unwrap();
        let mut pose = Pose::default();
        clip.sample(&skel, t, &mut pose);
        pose.joints[hips].t.y
    };
    assert!((sample(chan(Interpolation::Linear, vec![0.0, 0.0, 0.0, 0.0, 2.0, 0.0]), 0.5) - 1.0).abs() < 1e-5);
    assert!((sample(chan(Interpolation::Step, vec![0.0, 0.0, 0.0, 0.0, 2.0, 0.0]), 0.9) - 0.0).abs() < 1e-5);
    // Cubic: (in, value, out) per key, tangents zero.
    let cubic = vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
    assert!((sample(chan(Interpolation::CubicSpline, cubic.clone()), 0.5) - 0.5).abs() < 1e-5);
    assert!((sample(chan(Interpolation::CubicSpline, cubic), 0.25) - 0.15625).abs() < 1e-5);
    // A channel whose values do not match its keys is an error, not a panic.
    let bad = AnimClipData { name: "bad".into(), channels: vec![chan(Interpolation::Linear, vec![0.0, 1.0])] };
    assert!(SourceClip::from_data(&bad, &skel).is_err());
}

/// THE RETARGET TEST: a clip made on the foreign rig (A-pose, rolled bone frames, a 0.01 armature,
/// 1.1 times taller) plays on the T-pose body with each mapped bone pointing where the clip pointed
/// it, and the hips' bob scaled by the ratio of hips heights.
/// Seen red 2026-11-08 three ways, each on its own: composing the turn in each bone's local frame
/// instead of model space (the upper arm 1.81 degrees off), leaving out the rest alignment `C` (45.00
/// degrees off, the A-pose against the T-pose), and leaving the bob unscaled (0.05 m, not 0.0455).
#[test]
fn a_clip_made_on_a_foreign_rig_points_the_bodys_bones_where_it_pointed_them() {
    let h = humanoid();
    let (fj, fmap) = foreign_rig();
    let (bj, bmap) = body_rig();
    let src = Skeleton::from_skin(&fj, &fmap, &h).unwrap();
    let dst = Skeleton::from_skin(&bj, &bmap, &h).unwrap();
    let ua = src.joint_by_role(&h, "leftUpperArm").unwrap();
    let shoulder = src.parents[ua].unwrap();
    // A world-space turn of the left upper arm, written as local keys in the foreign rig's frames.
    let d = Quat::from_rotation_z(0.6) * Quat::from_rotation_y(-0.4);
    let w_parent = foreign_rest_rotation("leftShoulder");
    let local_rest = Quat::from_array(fj[ua].rotation);
    let local_turned = (w_parent.inverse() * d * foreign_rest_rotation("leftUpperArm")).normalize();
    let mut values: Vec<f32> = local_rest.to_array().to_vec();
    values.extend_from_slice(&local_turned.to_array());
    let hips = src.joint_by_role(&h, "hips").unwrap();
    let rest_t = Vec3::from(fj[hips].translation);
    let mut tvals: Vec<f32> = rest_t.to_array().to_vec();
    tvals.extend_from_slice(&(rest_t + Vec3::new(0.0, 5.0, 0.0)).to_array()); // 5 cm in the armature's units
    let data = AnimClipData {
        name: "raise".into(),
        channels: vec![
            AnimChannelData { node: fj[ua].name.clone(), path: ChannelPath::Rotation, interpolation: Interpolation::Linear, times: vec![0.0, 1.0], values },
            AnimChannelData { node: fj[hips].name.clone(), path: ChannelPath::Translation, interpolation: Interpolation::Linear, times: vec![0.0, 1.0], values: tvals },
        ],
    };
    let source = SourceClip::from_data(&data, &src).unwrap();
    let clip = retarget(&source, &src, &dst, &h, 30.0, false, 0.0);
    // Where the source's upper arm points at the end of the clip.
    let mut sp = Pose::default();
    let mut sm = Vec::new();
    source.sample(&src, 1.0, &mut sp);
    model_matrices(&src, &sp, &mut sm);
    let el = src.joint_by_role(&h, "leftLowerArm").unwrap();
    let src_dir = sm[el].w_axis.truncate() - sm[ua].w_axis.truncate();
    let _ = shoulder;
    // Where the body's points.
    let mut dp = Pose::default();
    let mut dm = Vec::new();
    clip.sample(&dst, 1.0, &mut dp);
    model_matrices(&dst, &dp, &mut dm);
    let (dua, del) = (dst.joint_by_role(&h, "leftUpperArm").unwrap(), dst.joint_by_role(&h, "leftLowerArm").unwrap());
    let dst_dir = dm[del].w_axis.truncate() - dm[dua].w_axis.truncate();
    assert!(angle_deg(src_dir, dst_dir) < 0.5, "upper arm {:.2} degrees off", angle_deg(src_dir, dst_dir));
    // At the clip's start (rest on the source, an A-pose) the T-pose body's arm comes down to it.
    clip.sample(&dst, 0.0, &mut dp);
    model_matrices(&dst, &dp, &mut dm);
    let start = dm[del].w_axis.truncate() - dm[dua].w_axis.truncate();
    assert!((angle_deg(start, Vec3::X) - 45.0).abs() < 0.5, "the arm starts {:.1} degrees below level", angle_deg(start, Vec3::X));
    // The bob: 5 cm on a rig whose hips stand 1.1 m up is 4.55 cm on the 1.0 m body.
    clip.sample(&dst, 1.0, &mut dp);
    model_matrices(&dst, &dp, &mut dm);
    let dh = dst.joint_by_role(&h, "hips").unwrap();
    let bob = dm[dh].w_axis.y - dst.rest_position(dh).y;
    assert!((bob - 0.05 / 1.1).abs() < 1e-3, "bob {bob}");
    // Joints the clip does not move stay at rest.
    let head = dst.joint_by_role(&h, "head").unwrap();
    assert!((dm[head].w_axis.truncate() - (dst.rest_position(head) + Vec3::new(0.0, bob, 0.0))).length() < 1e-3);
}

/// A twist joint takes its share of the wrist's twist about the forearm and none of its swing.
/// Seen red 2026-11-08 with `apply_twists` copying the hand's whole rotation (swing included).
#[test]
fn a_twist_joint_takes_its_share_of_the_wrists_twist_and_none_of_its_swing() {
    let h = humanoid();
    let (joints, map) = body_rig();
    let skel = Skeleton::from_skin(&joints, &map, &h).unwrap();
    let hand = skel.joint_by_role(&h, "leftHand").unwrap();
    let twist = skel.joint_by_role(&h, "leftLowerArmTwist").unwrap();
    let share = h.roles[h.role_index("leftLowerArmTwist").unwrap()].twist.as_ref().unwrap().share;
    let mut pose = skel.rest_pose();
    // 80 degrees of twist about the forearm (+X in a T-pose) and 30 of swing about +Z.
    pose.joints[hand].r = Quat::from_rotation_z(0.0) * Quat::from_rotation_x(80f32.to_radians());
    let swing = Quat::from_rotation_z(30f32.to_radians());
    pose.joints[hand].r = swing * pose.joints[hand].r;
    apply_twists(&skel, &h, &mut pose);
    let (axis, angle) = pose.joints[twist].r.to_axis_angle();
    assert!((angle.to_degrees() - 80.0 * share).abs() < 2.0, "twist joint turned {:.1} degrees", angle.to_degrees());
    assert!(axis.abs().dot(Vec3::X) > 0.99, "about {axis}");
}

/// The blend space's two neighbours and weight, and stride warping's rate: the body's speed over
/// the clip's, within limits, negative backwards.
/// Seen red 2026-11-08 with `stride_rate` ignoring the body's direction (a backwards walk played
/// forwards).
#[test]
fn the_blend_space_brackets_its_input_and_strides_match_the_bodys_speed() {
    let s = [(0.0, 0), (1.4, 1), (3.5, 2)];
    assert_eq!(bracket(&s, -1.0), (0, 0, 0.0));
    let (a, b, w) = bracket(&s, 2.45);
    assert_eq!((a, b), (1, 2));
    assert!((w - 0.5).abs() < 1e-5);
    assert_eq!(bracket(&s, 9.0), (2, 2, 0.0));
    let st = Some(StrideDef { min_rate: 0.6, max_rate: 1.5 });
    assert!((stride_rate(st, 1.4, 1.4) - 1.0).abs() < 1e-6);
    assert!((stride_rate(st, 0.1, 1.4) - 0.6).abs() < 1e-6);
    assert!((stride_rate(st, 9.0, 1.4) - 1.5).abs() < 1e-6);
    assert!((stride_rate(st, -1.4, 1.4) + 1.0).abs() < 1e-6);
    assert_eq!(stride_rate(None, 3.0, 1.4), 1.0);
}

/// Synthetic clips that each turn the hips about +Y by their own angle, so a pose says which clips
/// it came from and in what share.
fn marker_clips(skel: &Skeleton, h: &crate::HumanoidDef) -> ClipSet {
    let hips = skel.joint_by_role(h, "hips").unwrap();
    let mut set = ClipSet::default();
    for (id, deg, speed) in [("idle", 0.0, 0.0), ("walk", 10.0, 1.0), ("jog", 20.0, 5.9), ("sprint", 25.0, 8.9), ("turn_left", 30.0, 0.0), ("turn_right", -30.0, 0.0)] {
        let mut rotations = vec![None; skel.joint_count()];
        rotations[hips] = Some(vec![Quat::from_rotation_y(f32::to_radians(deg)); 31]);
        set.insert(id, Clip { name: id.into(), duration: 1.0, fps: 30.0, frames: 31, looped: true, speed_mps: speed, rotations, translations: vec![None; skel.joint_count()] });
    }
    set
}

fn hips_yaw_deg(pose: &Pose, skel: &Skeleton, h: &crate::HumanoidDef) -> f32 {
    let q = pose.joints[skel.joint_by_role(h, "hips").unwrap()].r;
    let (axis, angle) = q.to_axis_angle();
    angle.to_degrees() * axis.y.signum()
}

/// The SHIPPED graph (`data/characters/locomotion.ron`) on marker clips: standing is idle, a turn on
/// the spot is the turn space (left positive), moving between the walk's and the jog's places blends
/// the two, the realistic walk (1.4 m/s) is the walk alone, and stopping fades back to idle.
/// Seen red 2026-11-08 with the move state's transition reading `AbsBelow` for `AbsAbove` in the data;
/// and 2026-11-09 with the walk placed at 0.98 (its own speed): "the realistic walk is the walk
/// alone: 11.389631", a seventh jog.
#[test]
fn the_shipped_graph_idles_turns_walks_runs_and_fades_back() {
    let h = humanoid();
    let (joints, map) = body_rig();
    let skel = Skeleton::from_skin(&joints, &map, &h).unwrap();
    let clips = marker_clips(&skel, &h);
    let text = std::fs::read_to_string(concat!(env!("HUMANITY_REPO_ROOT"), "/data/characters/locomotion.ron")).unwrap();
    let graph = Graph::resolve(&Graph::parse(&text).unwrap(), &clips).expect("the shipped graph resolves");
    let mut anim = Animator::new(&graph);
    let mut pose = Pose::default();
    let run = |anim: &mut Animator, input: LocomotionInput, secs: f32, pose: &mut Pose| {
        let steps = (secs / 0.05) as usize;
        for _ in 0..steps {
            anim.update(&graph, &clips, &skel, &h, &input, 0.05, pose);
        }
    };
    run(&mut anim, LocomotionInput::default(), 1.0, &mut pose);
    assert_eq!(graph.state_ids[anim.state()], "idle");
    assert!(hips_yaw_deg(&pose, &skel, &h).abs() < 0.5);
    run(&mut anim, LocomotionInput { turn_rate: 2.0, ..Default::default() }, 1.0, &mut pose);
    assert_eq!(graph.state_ids[anim.state()], "turn");
    let turning = hips_yaw_deg(&pose, &skel, &h);
    assert!(turning > 5.0 && turning <= 30.0, "turning left reads {turning}");
    run(&mut anim, LocomotionInput { speed: 2.8, ..Default::default() }, 1.5, &mut pose);
    assert_eq!(graph.state_ids[anim.state()], "move");
    assert!(!anim.is_fading());
    let moving = hips_yaw_deg(&pose, &skel, &h);
    assert!(moving > 10.5 && moving < 19.5, "between walk and jog reads {moving}");
    run(&mut anim, LocomotionInput { speed: 1.4, ..Default::default() }, 1.5, &mut pose);
    let walking = hips_yaw_deg(&pose, &skel, &h);
    assert!((walking - 10.0).abs() < 0.3, "the realistic walk is the walk alone: {walking}");
    run(&mut anim, LocomotionInput::default(), 1.5, &mut pose);
    assert_eq!(graph.state_ids[anim.state()], "idle");
    assert!(hips_yaw_deg(&pose, &skel, &h).abs() < 0.5);
}

/// Speed along the facing, sideways speed and turn rate, from positions alone; a teleport resets
/// instead of reading as a sprint.
/// Seen red 2026-11-08 with the forward vector written (cos, 0, sin) (a body facing +X read as
/// walking sideways).
#[test]
fn the_motion_tracker_reads_speed_and_turn_from_positions_and_ignores_teleports() {
    let mut m = MotionTracker::default();
    let facing = std::f32::consts::FRAC_PI_2; // facing +X
    let mut input = LocomotionInput::default();
    for i in 0..120 {
        let p = Vec3::new(1.4 * i as f32 / 60.0, 0.0, 0.0);
        input = m.observe(p, facing, 1.0 / 60.0, 0.15, 12.0);
    }
    assert!((input.speed - 1.4).abs() < 0.01, "speed {}", input.speed);
    assert!(input.side.abs() < 0.01);
    for i in 0..120 {
        input = m.observe(Vec3::new(2.8, 0.0, 0.0), facing + i as f32 / 60.0, 1.0 / 60.0, 0.15, 12.0);
    }
    assert!((input.turn_rate - 1.0).abs() < 0.02, "turn {}", input.turn_rate);
    assert!(input.speed.abs() < 0.05);
    input = m.observe(Vec3::new(500.0, 0.0, 0.0), facing + 2.0, 1.0 / 60.0, 0.15, 12.0);
    assert_eq!(input, LocomotionInput::default());
}

/// The nearest bodies take the near places up to the cap and within the near distance; every
/// other body is middle.
/// Seen red 2026-11-08 with the bodies taken in list order rather than by distance.
#[test]
fn the_nearest_bodies_take_the_near_places_up_to_the_cap() {
    let tiers = Tiers { near_m: 15.0, middle_m: 40.0, near_cap: 2 };
    let got = assign(&[30.0, 3.0, 10.0, 5.0, 14.0], &tiers);
    assert_eq!(got, vec![DrawTier::Middle, DrawTier::Near, DrawTier::Middle, DrawTier::Near, DrawTier::Middle]);
    let got = assign(&[16.0, 2.0], &Tiers { near_cap: 9, ..tiers });
    assert_eq!(got, vec![DrawTier::Middle, DrawTier::Near]);
}

/// Every shipped role map reads against the standard (MakeHuman's, SOMA's 30 joints, the test
/// rigs'), SOMA's 30 joints all play a role, and the written-out MakeHuman-to-SOMA joint map agrees
/// with the two role maps joint for joint (the operator, 2026-11-08).
/// Seen red 2026-11-08 with Neck2 mapped to neck01 in the joint map ("Neck2 plays upperNeck but
/// neck01 plays Some(\"neck\")").
#[test]
fn the_shipped_role_maps_read_and_makehuman_maps_onto_soma_joint_for_joint() {
    let h = humanoid();
    let dir = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data/characters");
    let read = |rel: &str| std::fs::read_to_string(dir.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
    for entry in std::fs::read_dir(dir.join("role_maps")).unwrap() {
        let p = entry.unwrap().path();
        let text = std::fs::read_to_string(&p).unwrap();
        RoleMap::parse(&text, &h).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    }
    let mh = RoleMap::parse(&read("role_maps/makehuman_default.ron"), &h).unwrap();
    let soma = RoleMap::parse(&read("role_maps/soma_30.ron"), &h).unwrap();
    assert_eq!(soma.bones.len(), 30, "SOMA's layout has 30 joints, every one mapped");
    let map = crate::joint_map::JointMapDef::parse(&read("joint_maps/makehuman_to_soma30.ron")).unwrap();
    assert_eq!((map.from.as_str(), map.to.as_str()), ("makehuman_default", "soma_30"));
    let problems = map.check(&mh, &soma, &h);
    assert!(problems.is_empty(), "{problems:#?}");
    // Every required role of the standard has a MakeHuman bone.
    for r in h.roles.iter().filter(|r| r.required) {
        assert!(mh.bones.values().any(|v| v == &r.role), "MakeHuman has no bone for {}", r.role);
    }
}

/// A model matrix helper used above stays an affine transform (the palette assumes it).
#[test]
fn palette_rows_of_an_affine_matrix_round_trip() {
    let m = Mat4::from_scale_rotation_translation(Vec3::splat(1.3), Quat::from_rotation_x(0.7), Vec3::new(1.0, 2.0, 3.0));
    let rows = crate::pose::affine_rows(&m);
    let p = Vec3::new(-0.4, 0.2, 0.9);
    assert!((apply_rows(&rows, p) - m.transform_point3(p)).length() < 1e-5);
}

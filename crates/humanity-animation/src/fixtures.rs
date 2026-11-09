//! Rigs built in code for the tests: a small T-pose humanoid, and a "foreign" rig with the same
//! roles under other bone names, other proportions, an A-pose, every bone's local axes rolled a
//! different way and Mixamo's arrangement of a 0.01-scaled armature with centimetre joints. If a
//! clip made on the second plays right on the first, retargeting by role holds.

use glam::{Mat4, Quat, Vec3};
use humanity_core::skin_data::SkinJointData;

use crate::{HumanoidDef, RoleMap};

/// The shipped standard, `data/characters/skeletons/humanoid.ron`.
pub fn humanoid() -> HumanoidDef {
    let path = concat!(env!("HUMANITY_REPO_ROOT"), "/data/characters/skeletons/humanoid.ron");
    let text = std::fs::read_to_string(path).expect("data/characters/skeletons/humanoid.ron");
    HumanoidDef::parse(&text).expect("the shipped humanoid standard parses")
}

/// (role, parent role, offset from the parent in a T-pose, metres).
const BODY: &[(&str, Option<&str>, [f32; 3])] = &[
    ("hips", None, [0.0, 1.0, 0.0]),
    ("spine", Some("hips"), [0.0, 0.1, 0.0]),
    ("chest", Some("spine"), [0.0, 0.2, 0.0]),
    ("neck", Some("chest"), [0.0, 0.25, 0.0]),
    ("head", Some("neck"), [0.0, 0.1, 0.0]),
    ("leftShoulder", Some("chest"), [0.05, 0.2, 0.0]),
    ("leftUpperArm", Some("leftShoulder"), [0.12, 0.0, 0.0]),
    ("leftLowerArm", Some("leftUpperArm"), [0.28, 0.0, 0.0]),
    ("leftHand", Some("leftLowerArm"), [0.25, 0.0, 0.0]),
    ("leftLowerArmTwist", Some("leftLowerArm"), [0.125, 0.0, 0.0]),
    ("rightShoulder", Some("chest"), [-0.05, 0.2, 0.0]),
    ("rightUpperArm", Some("rightShoulder"), [-0.12, 0.0, 0.0]),
    ("rightLowerArm", Some("rightUpperArm"), [-0.28, 0.0, 0.0]),
    ("rightHand", Some("rightLowerArm"), [-0.25, 0.0, 0.0]),
    ("leftUpperLeg", Some("hips"), [0.1, -0.05, 0.0]),
    ("leftLowerLeg", Some("leftUpperLeg"), [0.0, -0.45, 0.0]),
    ("leftFoot", Some("leftLowerLeg"), [0.0, -0.42, 0.0]),
    ("rightUpperLeg", Some("hips"), [-0.1, -0.05, 0.0]),
    ("rightLowerLeg", Some("rightUpperLeg"), [0.0, -0.45, 0.0]),
    ("rightFoot", Some("rightLowerLeg"), [0.0, -0.42, 0.0]),
];

fn index_of(role: &str) -> usize {
    BODY.iter().position(|b| b.0 == role).expect("a fixture role")
}

/// World rest positions of the fixture's joints, with the arms turned `arm_drop_deg` down from
/// level (0 = T-pose, 45 = A-pose) and the whole figure scaled by `scale`.
fn positions(scale: f32, arm_drop_deg: f32) -> Vec<Vec3> {
    let mut out: Vec<Vec3> = Vec::with_capacity(BODY.len());
    for (role, parent, off) in BODY {
        let mut o = Vec3::from(*off) * scale;
        let in_arm = role.contains("Arm") || role.contains("Hand");
        if in_arm && !role.contains("Shoulder") {
            let side = o.x.signum();
            let drop = Quat::from_rotation_z(-side * arm_drop_deg.to_radians());
            o = drop * o;
        }
        let p = parent.map(|pr| out[index_of(pr)]).unwrap_or(Vec3::ZERO);
        out.push(p + o);
    }
    out
}

/// Build a skin from world rest positions and rotations: each joint's local transform is its
/// parent's model matrix inverted times its own, the inverse bind is its own model matrix
/// inverted, and the root hangs under `outer`.
fn skin(names: &[String], pos: &[Vec3], rot: &[Quat], joint_scale: f32, outer: Mat4) -> Vec<SkinJointData> {
    let model: Vec<Mat4> = (0..BODY.len()).map(|i| Mat4::from_scale_rotation_translation(Vec3::splat(joint_scale), rot[i], pos[i])).collect();
    (0..BODY.len())
        .map(|i| {
            let parent = BODY[i].1.map(index_of);
            let parent_model = parent.map(|p| model[p]).unwrap_or(outer);
            let local = parent_model.inverse() * model[i];
            let (s, r, t) = local.to_scale_rotation_translation();
            SkinJointData {
                name: names[i].clone(),
                parent,
                translation: t.to_array(),
                rotation: r.normalize().to_array(),
                scale: s.to_array(),
                inverse_bind: model[i].inverse().to_cols_array_2d(),
                outer: if parent.is_none() && outer != Mat4::IDENTITY { Some(outer.to_cols_array_2d()) } else { None },
            }
        })
        .collect()
}

/// The body rig: T-pose, metres, bone names are the role names with a `body_` prefix, every
/// local rotation the identity.
pub fn body_rig() -> (Vec<SkinJointData>, RoleMap) {
    let names: Vec<String> = BODY.iter().map(|b| format!("body_{}", b.0)).collect();
    let pos = positions(1.0, 0.0);
    let rot = vec![Quat::IDENTITY; BODY.len()];
    let joints = skin(&names, &pos, &rot, 1.0, Mat4::IDENTITY);
    (joints, role_map("fixture_body", &names))
}

/// The foreign rig: 1.1 times taller, A-pose, every joint's rest frame rolled its own way, a
/// 0.01 armature with centimetre joints, and Mixamo-style names.
pub fn foreign_rig() -> (Vec<SkinJointData>, RoleMap) {
    let names: Vec<String> = BODY.iter().map(|b| format!("mixamorig:{}", b.0)).collect();
    let pos = positions(1.1, 45.0);
    let rot: Vec<Quat> = (0..BODY.len())
        .map(|i| Quat::from_axis_angle(Vec3::new(1.0, 2.0 + i as f32, 0.5).normalize(), 0.4 + 0.37 * i as f32))
        .collect();
    let joints = skin(&names, &pos, &rot, 0.01, Mat4::from_scale(Vec3::splat(0.01)));
    (joints, role_map("fixture_foreign", &names))
}

fn role_map(id: &str, names: &[String]) -> RoleMap {
    RoleMap {
        id: id.to_string(),
        description: String::new(),
        facing_yaw_deg: 0.0,
        bones: names.iter().zip(BODY).map(|(n, b)| (n.clone(), b.0.to_string())).collect(),
    }
}

/// The world-space rest rotation the foreign rig gave a role (to author keys in its frames).
pub fn foreign_rest_rotation(role: &str) -> Quat {
    let i = index_of(role);
    Quat::from_axis_angle(Vec3::new(1.0, 2.0 + i as f32, 0.5).normalize(), 0.4 + 0.37 * i as f32)
}

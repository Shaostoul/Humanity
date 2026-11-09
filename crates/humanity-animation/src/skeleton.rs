//! A skeleton: one rig's joints as the file gave them (the skin's joint order, which is the order
//! of the pose palette and of the vertices' joint indices), with each joint's humanoid role from the
//! rig's role map.

use glam::{Mat4, Quat, Vec3};
use humanity_core::skin_data::SkinJointData;

use crate::pose::{model_matrices, Pose, Trs};
use crate::{HumanoidDef, RoleMap};

#[derive(Debug, Clone, PartialEq)]
pub struct Skeleton {
    /// The role map's id: which rig this is.
    pub rig: String,
    pub names: Vec<String>,
    pub parents: Vec<Option<usize>>,
    pub rest: Vec<Trs>,
    pub inverse_bind: Vec<Mat4>,
    /// A root joint's ancestor chain (identity for every joint with a joint parent), with the role
    /// map's facing turn folded in, so model space always faces +Z.
    pub outer: Vec<Mat4>,
    /// Joint indices, parents before children: the order poses are carried up the hierarchy in.
    pub order: Vec<usize>,
    /// Each joint's role, as an index into the humanoid standard's roles.
    pub roles: Vec<Option<usize>>,
    /// Each role's joint (indexed like the standard's roles).
    pub joint_of_role: Vec<Option<usize>>,
    /// Model-space rest matrices.
    pub rest_model: Vec<Mat4>,
}

impl Skeleton {
    /// Build a skeleton from a skin's joints and the rig's role map, and check it against the
    /// standard: every required role present, and each mapped joint's nearest mapped ancestor
    /// playing a role that is an ancestor of its own role in the standard (so a role map that
    /// swaps two limbs, or hangs an arm off a leg, is refused rather than drawn wrongly).
    pub fn from_skin(joints: &[SkinJointData], map: &RoleMap, humanoid: &HumanoidDef) -> Result<Self, String> {
        let n = joints.len();
        if n == 0 {
            return Err(format!("rig {}: the skin has no joints", map.id));
        }
        for (i, j) in joints.iter().enumerate() {
            if let Some(p) = j.parent {
                if p >= n || p == i {
                    return Err(format!("rig {}: joint {} has parent {p}, which is not another joint", map.id, j.name));
                }
            }
        }
        let parents: Vec<Option<usize>> = joints.iter().map(|j| j.parent).collect();
        let order = parents_first(&parents).ok_or_else(|| format!("rig {}: the joints' parents go round in a loop", map.id))?;
        let facing = Mat4::from_quat(Quat::from_rotation_y(map.facing_yaw_deg.to_radians()));
        let outer: Vec<Mat4> = joints
            .iter()
            .map(|j| match j.parent {
                Some(_) => Mat4::IDENTITY,
                None => facing * j.outer.map(|m| Mat4::from_cols_array_2d(&m)).unwrap_or(Mat4::IDENTITY),
            })
            .collect();
        let mut roles = vec![None; n];
        let mut joint_of_role = vec![None; humanoid.roles.len()];
        for (i, j) in joints.iter().enumerate() {
            if let Some(role) = map.role_of(&j.name) {
                let r = humanoid.role_index(role).expect("RoleMap::parse checked every role");
                roles[i] = Some(r);
                joint_of_role[r] = Some(i);
            }
        }
        for (r, def) in humanoid.roles.iter().enumerate() {
            if def.required && joint_of_role[r].is_none() {
                return Err(format!("rig {}: no bone plays the required role {}", map.id, def.role));
            }
        }
        // Hierarchy check by role.
        for i in 0..n {
            let Some(r) = roles[i] else { continue };
            let mut up = parents[i];
            while let Some(p) = up {
                if roles[p].is_some() {
                    break;
                }
                up = parents[p];
            }
            if let Some(p) = up {
                let pr = roles[p].expect("stopped on a mapped joint");
                if !role_is_ancestor(humanoid, pr, r) {
                    return Err(format!(
                        "rig {}: bone {} plays {} but hangs under {} ({}), which is not above {} in the humanoid standard",
                        map.id, joints[i].name, humanoid.roles[r].role, joints[p].name, humanoid.roles[pr].role, humanoid.roles[r].role
                    ));
                }
            }
        }
        let rest: Vec<Trs> = joints.iter().map(|j| Trs::from_arrays(j.translation, j.rotation, j.scale)).collect();
        let mut skel = Skeleton {
            rig: map.id.clone(),
            names: joints.iter().map(|j| j.name.clone()).collect(),
            parents,
            rest,
            inverse_bind: joints.iter().map(|j| Mat4::from_cols_array_2d(&j.inverse_bind)).collect(),
            outer,
            order,
            roles,
            joint_of_role,
            rest_model: Vec::new(),
        };
        let rest_pose = skel.rest_pose();
        let mut rest_model = Vec::new();
        model_matrices(&skel, &rest_pose, &mut rest_model);
        skel.rest_model = rest_model;
        Ok(skel)
    }

    pub fn joint_count(&self) -> usize {
        self.names.len()
    }

    /// This skeleton with each joint's rest position moved by `moves` (model space, one per joint;
    /// a joint past the list's end stays put): what a body's morph targets do to its skeleton, so a
    /// taller body's knees are where its taller legs bend (2026-11-09). Rest rotations and scales are
    /// kept; each joint's rest translation is worked out again under its parent, and its inverse
    /// bind is its new rest matrix inverted, so the reshaped mesh, skinned at rest, stands exactly
    /// as it was shaped. Clips baked on the unshaped skeleton play on it unchanged: they carry
    /// rotations, and the hips' travel as an offset from the rest (`clip`).
    pub fn reshaped(&self, moves: &[Vec3]) -> Skeleton {
        let mut out = self.clone();
        let wanted: Vec<Vec3> = (0..self.joint_count()).map(|j| self.rest_position(j) + moves.get(j).copied().unwrap_or(Vec3::ZERO)).collect();
        let mut model = vec![Mat4::IDENTITY; self.joint_count()];
        for &j in &self.order {
            let parent = match self.parents[j] {
                Some(p) => model[p],
                None => self.outer[j],
            };
            out.rest[j].t = parent.inverse().transform_point3(wanted[j]);
            model[j] = parent * out.rest[j].matrix();
        }
        out.inverse_bind = model.iter().map(|m| m.inverse()).collect();
        out.rest_model = model;
        out
    }

    /// The rest pose: every joint's own rest transform.
    pub fn rest_pose(&self) -> Pose {
        Pose { joints: self.rest.clone() }
    }

    /// The joint playing a role, by the role's index in the standard.
    pub fn joint(&self, role: usize) -> Option<usize> {
        self.joint_of_role.get(role).copied().flatten()
    }

    /// The joint playing a role, by name.
    pub fn joint_by_role(&self, humanoid: &HumanoidDef, role: &str) -> Option<usize> {
        self.joint(humanoid.role_index(role)?)
    }

    /// A joint's position in model space at rest.
    pub fn rest_position(&self, joint: usize) -> Vec3 {
        self.rest_model[joint].w_axis.truncate()
    }

    /// A joint's orientation in model space at rest (the rotation part of its model matrix).
    pub fn rest_world_rotation(&self, joint: usize) -> Quat {
        let (_, r, _) = self.rest_model[joint].to_scale_rotation_translation();
        r.normalize()
    }

    /// How high the hips stand at rest, metres: the retargeter scales root travel by the ratio of
    /// two rigs' hips heights (leg length, plan 2.1). Falls back to 1 m if the hips are on the floor.
    pub fn hips_height(&self, humanoid: &HumanoidDef) -> f32 {
        let h = self.joint_by_role(humanoid, "hips").map(|j| self.rest_position(j).y).unwrap_or(1.0);
        if h > 0.05 {
            h
        } else {
            1.0
        }
    }
}

/// Is role `a` an ancestor of role `b` (or the same role) in the standard?
fn role_is_ancestor(humanoid: &HumanoidDef, a: usize, b: usize) -> bool {
    let mut at = Some(b);
    while let Some(r) = at {
        if r == a {
            return true;
        }
        at = humanoid.roles[r].parent.as_deref().and_then(|p| humanoid.role_index(p));
    }
    false
}

/// Joint indices with every parent before its children; None if the parents loop.
fn parents_first(parents: &[Option<usize>]) -> Option<Vec<usize>> {
    let n = parents.len();
    let mut placed = vec![false; n];
    let mut order = Vec::with_capacity(n);
    while order.len() < n {
        let before = order.len();
        for i in 0..n {
            if !placed[i] && parents[i].map_or(true, |p| placed[p]) {
                placed[i] = true;
                order.push(i);
            }
        }
        if order.len() == before {
            return None;
        }
    }
    Some(order)
}

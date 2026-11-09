//! A joint map: one rig's joints read straight across onto another's, joint for joint
//! (`data/characters/joint_maps/<from>_to_<to>.ron`).
//!
//! The first is MakeHuman to SOMA's 30 joints (the operator, 2026-11-08): the body is MakeHuman, and
//! its skeleton is also mapped to SOMA's layout so a SOMA-driven model (MotionBricks' class, plan
//! rung 7) can read and write it. The humanoid roles already imply the map through the two role
//! maps; this file writes it out for a model's input and output, and `check` holds it to the role
//! maps so the two cannot disagree.

use serde::Deserialize;

use crate::{HumanoidDef, RoleMap};

/// Where a target joint stands on the source rig.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum Source {
    /// The source bone whose joint is the target joint.
    Bone(String),
    /// The far end (tail) of the source bone: its joint plus the bone's length along it.
    TipOf(String),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct JointPair {
    pub soma: String,
    pub makehuman: Source,
}

/// `data/characters/joint_maps/makehuman_to_soma30.ron`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct JointMapDef {
    /// The source rig's role map id.
    pub from: String,
    /// The target rig's role map id.
    pub to: String,
    pub joints: Vec<JointPair>,
}

impl JointMapDef {
    pub fn parse(text: &str) -> Result<Self, String> {
        crate::parse_ron(text, "joint map")
    }

    /// Hold the map to the two role maps: every target joint listed exactly once; a `Bone` plays
    /// the same role on the source as the target joint does on the target; a `TipOf` bone plays
    /// the role directly above the target joint's (the tip's distal bone). Returns every
    /// disagreement.
    pub fn check(&self, from: &RoleMap, to: &RoleMap, humanoid: &HumanoidDef) -> Vec<String> {
        let mut problems = Vec::new();
        for joint in to.bones.keys() {
            let n = self.joints.iter().filter(|p| &p.soma == joint).count();
            if n != 1 {
                problems.push(format!("{} is listed {n} times, not once", joint));
            }
        }
        for p in &self.joints {
            let Some(role) = to.role_of(&p.soma) else {
                problems.push(format!("{} is not a joint of {}", p.soma, to.id));
                continue;
            };
            match &p.makehuman {
                Source::Bone(b) => match from.role_of(b) {
                    Some(r) if r == role => {}
                    other => problems.push(format!("{} plays {role} but {b} plays {other:?}", p.soma)),
                },
                Source::TipOf(b) => {
                    let above = humanoid.role_index(role).and_then(|i| humanoid.roles[i].parent.clone());
                    if from.role_of(b) != above.as_deref() {
                        problems.push(format!("{} is the tip of {role}'s bone {above:?}, but {b} plays {:?}", p.soma, from.role_of(b)));
                    }
                }
            }
        }
        problems
    }
}

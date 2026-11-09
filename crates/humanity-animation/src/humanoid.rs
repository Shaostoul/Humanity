//! The humanoid skeleton standard: the roles a humanoid body's joints can play, read from
//! `data/characters/skeletons/humanoid.ron` (plan section 2.1).
//!
//! The role names are VRM 1.0's humanoid bone names (https://github.com/vrm-c/vrm-specification,
//! `specification/VRMC_vrm-1.0/humanoid.md`), adopted as a vocabulary and not as a file format: an
//! open, documented naming for humanoids in glTF, so any rig whose bones a role map names that way
//! maps by role. A few roles are ours (`standard: Hos`): the twist joints, which VRM does not list
//! and linear blend skinning needs (plan 2.1: "Twist joints for the forearms, upper arms and
//! thighs").
//!
//! Each role says its parent role (for the hierarchy check and the retargeter), the lowest drawing
//! tier it belongs to (rung 4's middle and far tiers draw fewer joints), whether a body must have it,
//! the role its bone points at (for aligning rest poses that differ, an A-pose against a T-pose),
//! and, for a twist joint, whose twist it takes and how much. Sockets (where a prop attaches) are
//! rows too, so a prop names a socket, never a bone.

use serde::Deserialize;

/// The lowest drawing tier a joint belongs to (plan section 8.4). Rung 1 draws every joint in both
/// the near and the middle tier; rung 4 drops `Near`-only joints in the middle tier and bakes the
/// far tier from the `Far` set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub enum Tier {
    /// Only up close: fingers, eyes, the jaw.
    Near,
    /// Near and middle: the neck, the shoulders, the hands, the toes.
    Middle,
    /// Every tier, the far bake included: the core of the body.
    Far,
}

/// Where a role's name comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
pub enum Standard {
    /// VRM 1.0's humanoid bone list.
    #[default]
    Vrm,
    /// Ours: twist joints and anything else VRM leaves out.
    Hos,
}

/// A twist joint's driver: the role whose twist about its bone it shares, and the share (0 to 1).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TwistDef {
    pub of: String,
    pub share: f32,
}

/// One role of the standard.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RoleDef {
    pub role: String,
    pub parent: Option<String>,
    pub tier: Tier,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub standard: Standard,
    /// The role whose joint this bone points at, at rest: the bone's direction is from this joint
    /// to that one. Used to align two rigs' rest poses when retargeting (`align`).
    #[serde(default)]
    pub aim: Option<String>,
    /// Align the bone's rest direction between rigs when retargeting (limbs: so a clip made on a
    /// T-pose rig plays on an A-pose body with its arms where the clip put them).
    #[serde(default)]
    pub align: bool,
    #[serde(default)]
    pub twist: Option<TwistDef>,
}

/// A named place on the body a prop attaches to (rung 2's grips use these).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SocketDef {
    pub id: String,
    /// The role it rides on.
    pub role: String,
    /// Offset from that joint, metres, in the character's rest frame (+z forward, +y up, +x the
    /// character's left), turned with the joint at run time; so it is the same for every rig.
    pub offset_m: [f32; 3],
}

/// The whole standard: `data/characters/skeletons/humanoid.ron`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct HumanoidDef {
    pub id: String,
    pub roles: Vec<RoleDef>,
    #[serde(default)]
    pub sockets: Vec<SocketDef>,
}

impl HumanoidDef {
    /// Parse and check the file: unique roles, every parent, aim, twist source and socket naming a
    /// role, and no loop in the parents.
    pub fn parse(text: &str) -> Result<Self, String> {
        let def: HumanoidDef = crate::parse_ron(text, "humanoid skeleton")?;
        def.check()?;
        Ok(def)
    }

    /// The index of a role by name.
    pub fn role_index(&self, role: &str) -> Option<usize> {
        self.roles.iter().position(|r| r.role == role)
    }

    fn check(&self) -> Result<(), String> {
        for (i, r) in self.roles.iter().enumerate() {
            if self.roles[..i].iter().any(|o| o.role == r.role) {
                return Err(format!("humanoid skeleton: role {} is listed twice", r.role));
            }
        }
        let known = |name: &str| self.role_index(name).is_some();
        for r in &self.roles {
            for (what, other) in [("parent", r.parent.as_deref()), ("aim", r.aim.as_deref()), ("twist source", r.twist.as_ref().map(|t| t.of.as_str()))] {
                if let Some(o) = other {
                    if !known(o) {
                        return Err(format!("humanoid skeleton: role {}'s {what} {o} is not a role", r.role));
                    }
                }
            }
            if let Some(t) = &r.twist {
                if !(0.0..=1.0).contains(&t.share) {
                    return Err(format!("humanoid skeleton: role {}'s twist share {} is outside 0 to 1", r.role, t.share));
                }
            }
            // Walk up the parents: a loop never reaches a root.
            let mut at = r.parent.as_deref();
            let mut steps = 0;
            while let Some(p) = at {
                steps += 1;
                if steps > self.roles.len() {
                    return Err(format!("humanoid skeleton: the parents of {} go round in a loop", r.role));
                }
                at = self.roles[self.role_index(p).expect("checked above")].parent.as_deref();
            }
        }
        for s in &self.sockets {
            if !known(&s.role) {
                return Err(format!("humanoid skeleton: socket {} rides on {}, which is not a role", s.id, s.role));
            }
        }
        Ok(())
    }
}

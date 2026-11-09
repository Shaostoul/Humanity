//! A role map: one rig's bone names, each turned into a humanoid role
//! (`data/characters/role_maps/<id>.ron`, plan section 2.1).
//!
//! The bridge that makes the system body-agnostic: a MakeHuman body names its thigh
//! `upperleg01.L`, a Mixamo-style rig `mixamorig:LeftUpLeg`, our test figure `thigh.L`; each gets a
//! file that says all three are `leftUpperLeg`. Bones a map leaves out (extra spine segments, a
//! rig's own helpers) keep their rest pose under whatever their parent does. A rig authored facing
//! away from glTF's +Z gives its turn in `facing_yaw_deg`.

use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RoleMap {
    pub id: String,
    /// Plain words: which rig this is and where it comes from.
    #[serde(default)]
    pub description: String,
    /// Degrees about +Y that turn the rig to face +Z (glTF's front). 0 for a rig that already does.
    #[serde(default)]
    pub facing_yaw_deg: f32,
    /// Bone name in the file -> role name in the humanoid standard.
    pub bones: BTreeMap<String, String>,
}

impl RoleMap {
    /// Parse and check it against the standard: every role it names exists, and no role is given
    /// to two bones.
    pub fn parse(text: &str, humanoid: &crate::HumanoidDef) -> Result<Self, String> {
        let map: RoleMap = crate::parse_ron(text, "role map")?;
        let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
        for (bone, role) in &map.bones {
            if humanoid.role_index(role).is_none() {
                return Err(format!("role map {}: bone {bone} is given role {role}, which the humanoid standard does not have", map.id));
            }
            if let Some(other) = seen.insert(role.as_str(), bone.as_str()) {
                return Err(format!("role map {}: role {role} is given to both {other} and {bone}", map.id));
            }
        }
        Ok(map)
    }

    /// The role a bone plays, if the map names it.
    pub fn role_of(&self, bone: &str) -> Option<&str> {
        self.bones.get(bone).map(String::as_str)
    }
}

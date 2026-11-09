//! A model's physics, from the `physics.ron` beside it (docs/design/simready-props.md, section 6).
//!
//! The NVIDIA SimReady props carry real physics data authored with them (UsdPhysics: mass, centre
//! of mass, inertia, collision shapes, friction, joints); `just simready` (scripts/simready/pack.js)
//! writes it into `assets/models/simready/<model>/physics.ron`, in the model's own frame, and puts
//! the collider hulls in the model's `.glb` as meshes no scene draws. Any model may carry one: the
//! file sits beside the model, so a mod's model brings its own.
//!
//! What uses it today: the machine walk-up card's model credit line
//! (`engine::home_meshes::build_machine_bodies`, for a `package_id` from NVIDIA SimReady). Rapier's
//! world is still dormant and the home's collision is geometric (`humanity_ship::wall_collision`),
//! so the masses, shapes, friction and joints are kept for the rungs that wake it (design section 5,
//! rung 4: furniture that blocks walking, objects that can be knocked over or carried): each field
//! names the Rapier call it maps to.
//!
//! **The masses are NVIDIA's as published, unchecked.** Several are solid-volume estimates far from
//! the real object (a 503 kg steel shelf, a 2,035 kg medicine cabinet, a 0.18 kg drum), so nothing
//! shows them as facts: before a mass is used for carrying or on a card, it is checked against a
//! real source the way the plant data is (design section 6).
//!
//! Read fresh each time the home's bodies are built (a small file per model, read only on a
//! rebuild), so nothing here is kept between runs or can go stale.

use serde::Deserialize;
use std::path::Path;

/// One model's physics. Metres, kilograms, Y up, in the model's own frame (its footprint centred
/// on the origin, its base on y = 0).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ModelPhysics {
    /// The NVIDIA folder it came from, and the package (its version date) it was taken from.
    pub source: String,
    pub package_id: String,
    /// The whole object's mass: the sum of its bodies' (Rapier: `ColliderBuilder::mass`).
    pub mass_kg: Option<f32>,
    /// The mass-weighted centre of every part (Rapier: `MassProperties::local_com`).
    pub center_of_mass_m: Option<(f32, f32, f32)>,
    /// Its rigid bodies: a toaster's body, lever, dial and tray are four.
    pub bodies: Vec<ModelBody>,
    pub colliders: Vec<ModelCollider>,
    pub joints: Vec<ModelJoint>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ModelBody {
    pub name: String,
    pub mass_kg: Option<f32>,
    /// Moved by animation, not by forces (Rapier: `RigidBodyType::KinematicPositionBased`).
    pub kinematic: bool,
    /// Rapier: `MassProperties::with_principal_inertia_frame`.
    pub principal_inertia_kg_m2: Option<(f32, f32, f32)>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ModelCollider {
    pub body: String,
    pub part: String,
    /// UsdPhysics' word for the shape the author asked for: `convexHull`, `convexDecomposition`,
    /// `boundingCube`, `boundingSphere`, `meshSimplification` or `none` (the triangle mesh).
    pub approximation: String,
    /// The index of its convex hull among the `.glb`'s meshes (Rapier: `SharedShape::convex_hull`).
    pub hull_mesh: Option<u32>,
    /// Dynamic friction: Rapier keeps one coefficient per collider.
    pub friction: Option<f32>,
    pub static_friction: Option<f32>,
    pub restitution: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ModelJoint {
    /// `PhysicsRevoluteJoint`, `PhysicsPrismaticJoint`, `PhysicsFixedJoint`, ...
    pub kind: String,
    pub body0: String,
    pub body1: String,
    /// The joint's own axis, X, Y or Z, in its local frame.
    pub axis: String,
    /// Its limits, in `units` ("deg" or "m"); None for a joint that turns or slides freely.
    pub lower: Option<f32>,
    pub upper: Option<f32>,
    pub units: String,
}

impl ModelPhysics {
    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    /// The `physics.ron` beside a model file, if it has one.
    pub fn beside(model_file: &Path) -> Option<Result<Self, String>> {
        let path = model_file.parent()?.join("physics.ron");
        let text = std::fs::read_to_string(&path).ok()?;
        Some(Self::parse(&text).map_err(|e| format!("{}: {e}", path.display())))
    }
}

#[cfg(test)]
mod tests {
    use super::ModelPhysics;
    use std::path::Path;

    /// Every shipped `physics.ron` parses, names its own folder, carries a positive mass, and points
    /// its hulls at meshes the model's GLB really holds. Seen red 2026-11-09 with the toaster's file
    /// given `mass_kg: Some(-1.0)` ("appliance_toaster_a01: a positive mass, not -1 kg") and, alone,
    /// with the kettle's first collider's `hull_mesh` raised to 99 ("...names mesh 99 of 6").
    #[test]
    fn every_shipped_physics_file_reads_and_points_at_real_hulls() {
        let root = Path::new(env!("HUMANITY_REPO_ROOT")).join("assets").join("models").join("simready");
        let Ok(dirs) = std::fs::read_dir(&root) else { return };
        let mut seen = 0;
        for d in dirs.flatten() {
            let dir = d.path();
            if !dir.is_dir() {
                continue;
            }
            let name = dir.file_name().unwrap().to_string_lossy().to_string();
            let glb = dir.join(format!("{name}.glb"));
            let p = ModelPhysics::beside(&glb).unwrap_or_else(|| panic!("{name}: no physics.ron")).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(p.source.to_lowercase(), name, "{name}: physics.ron names its own folder");
            let mass = p.mass_kg.unwrap_or_else(|| panic!("{name}: no mass"));
            assert!(mass > 0.0, "{name}: a positive mass, not {mass} kg");
            let (doc, _, _) = gltf::import(&glb).unwrap_or_else(|e| panic!("{name}: the GLB reads: {e}"));
            let meshes = doc.meshes().count() as u32;
            for c in &p.colliders {
                if let Some(h) = c.hull_mesh {
                    assert!(h >= 2 && h < meshes, "{name}: collider {} names mesh {h} of {meshes}", c.part);
                }
            }
            seen += 1;
        }
        assert!(seen > 0, "no models under {}", root.display());
    }

    /// EVERY CATALOGUE ROW THAT DRAWS A SIMREADY MODEL DECLARES ITS REAL HEIGHT: the row's `size`
    /// height is within 2% of the model's own (its first mesh's vertical extent), so the engine's
    /// fit-to-height scale (`GltfCpuMesh::scale_to_height`) is 1 and the model is drawn at the size
    /// NVIDIA measured, not stretched to a primitive's guess. Seen red 2026-11-09 with the toilet
    /// row's height left at the old box's 0.75 m against the model's 1.04 m.
    #[test]
    fn every_simready_row_declares_its_models_real_height() {
        let repo = Path::new(env!("HUMANITY_REPO_ROOT"));
        let mut checked = 0;
        for file in ["home.ron", "home_solo.ron", "cabin.ron", "ship.ron"] {
            let Ok(text) = std::fs::read_to_string(repo.join("data").join("machines").join(file)) else { continue };
            for (at, _) in text.match_indices("model: Some(\"assets/models/simready/") {
                let start = at + "model: Some(\"".len();
                let model = &text[start..start + text[start..].find('"').expect("a closing quote")];
                // The row's `size` is the last one before its model line.
                let row = &text[..at];
                let s = row.rfind("size: (").expect("a size before the model") + "size: (".len();
                let size: Vec<f32> = row[s..s + row[s..].find(')').unwrap()].split(',').map(|v| v.trim().parse().expect("a number")).collect();
                let (doc, buffers, _) = gltf::import(repo.join(model)).unwrap_or_else(|e| panic!("{model}: {e}"));
                let mesh = doc.meshes().next().expect("a mesh");
                let prim = mesh.primitives().next().expect("a primitive");
                let reader = prim.reader(|b| Some(&buffers[b.index()]));
                let (mut lo, mut hi) = (f32::MAX, f32::MIN);
                for p in reader.read_positions().expect("positions") {
                    lo = lo.min(p[1]);
                    hi = hi.max(p[1]);
                }
                let height = hi - lo;
                assert!(((size[1] / height) - 1.0).abs() < 0.02, "{file}: the row drawing {model} declares {} m tall; the model is {height:.3} m", size[1]);
                checked += 1;
            }
        }
        assert!(checked > 0, "no catalogue row draws a SimReady model");
    }
}

//! What a finished structure does when the player uses it (2026-09-27).
//!
//! A blueprint's `provides` names the function a built structure serves.
//! Until now two of them, `rest` (the bed) and `storage` (the chest), were
//! words on the Crafting page and nothing else: the playable assessment's
//! 2.4, "the construction sink terminates in a decorative box". Now:
//!
//! - `rest`: press E at it to sleep. The body's sleep need is
//!   `Vitals::energy`, which drains over about sixteen waking hours and
//!   leaves you `fatigued` below 25. A night in a bed refills it, and the
//!   world runs through the night while you sleep (the sleep pass in
//!   `systems::food`).
//! - `storage`: the structure is a container in the Inventory page's places
//!   tree, addressed by `storage_path(uid)`, so every existing way of moving
//!   items (drag onto its header, Stash to, Take to backpack, Move to) works
//!   on it, and its contents are saved with the rest of home storage.
//!
//! The mechanisms are code, a small closed set; WHICH structures carry them
//! is data. Any blueprint that says `provides: Some("rest")` is somewhere to
//! sleep, whatever its id, so a bunk or a hammock is a data edit.
//!
//! `shelter` (walls, roofs) is deliberately not here: what a shelter keeps
//! off you (wind, rain, the night sky) does not exist in the body model yet.
//! docs/FEATURES.md says what it needs.

use super::{BlueprintRegistry, Structure};
use crate::ecs::components::Transform;
use glam::Vec3;

/// What using a finished structure does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureUse {
    /// Somewhere to sleep (`provides: "rest"`).
    Sleep,
    /// A container for items (`provides: "storage"`).
    Store,
}

impl StructureUse {
    /// The use a blueprint's `provides` names, when it names one the player
    /// can use with E. `None` for everything else (a wall's `shelter`, a
    /// furnace's `smelting`, which works through the station gate instead).
    pub fn from_provides(provides: &str) -> Option<Self> {
        match provides {
            "rest" => Some(Self::Sleep),
            "storage" => Some(Self::Store),
            _ => None,
        }
    }

    /// The crosshair prompt for this use of a structure called `name`.
    pub fn prompt(self, name: &str) -> String {
        match self {
            Self::Sleep => format!("[E] sleep in the {name}"),
            Self::Store => format!("[E] open the {name}"),
        }
    }
}

/// The use of a finished structure, from the `provides` it was built with.
/// That is saved with it, so no registry is needed.
pub fn use_of(s: &Structure) -> Option<StructureUse> {
    s.provides.as_deref().and_then(StructureUse::from_provides)
}

/// A finished structure's display name: its blueprint's name, or the
/// blueprint id when the registry is missing or no longer has it.
pub fn display_name(s: &Structure, registry: Option<&BlueprintRegistry>) -> String {
    registry
        .and_then(|r| r.get(&s.blueprint_id))
        .map(|bp| bp.name.clone())
        .unwrap_or_else(|| s.blueprint_id.clone())
}

/// Distance along the ray `origin + t * dir` (dir unit length) to where it
/// enters a structure's box, or None when it misses. The box is exactly the
/// one the renderer draws: the unit box scaled by the transform, its BOTTOM
/// at `position.y` (`Mesh::box_xyz` is bottom-origin), rotated about that
/// point. An origin inside the box hits at 0.
pub fn ray_hits_box(origin: Vec3, dir: Vec3, tf: &Transform) -> Option<f32> {
    let inv = tf.rotation.inverse();
    let o = inv * (origin - tf.position);
    let d = inv * dir;
    let half = tf.scale * 0.5;
    let lo = Vec3::new(-half.x, 0.0, -half.z);
    let hi = Vec3::new(half.x, tf.scale.y, half.z);
    let (mut t0, mut t1) = (0.0_f32, f32::INFINITY);
    for a in 0..3 {
        if d[a].abs() < 1e-8 {
            if o[a] < lo[a] || o[a] > hi[a] {
                return None;
            }
            continue;
        }
        let (mut near, mut far) = ((lo[a] - o[a]) / d[a], (hi[a] - o[a]) / d[a]);
        if near > far {
            std::mem::swap(&mut near, &mut far);
        }
        t0 = t0.max(near);
        t1 = t1.min(far);
        if t0 > t1 {
            return None;
        }
    }
    Some(t0)
}

/// The finished structure the player is looking at, when it is usable and
/// within `reach` metres: the FIRST structure the look ray meets, so a wall
/// between you and a chest hides the chest. Scaffolds (`Construction`) are
/// not structures yet and are never usable.
pub fn looked_at(
    world: &hecs::World,
    eye: Vec3,
    dir: Vec3,
    reach: f32,
) -> Option<(hecs::Entity, StructureUse)> {
    let dir = dir.normalize_or_zero();
    if dir == Vec3::ZERO {
        return None;
    }
    let mut first: Option<(hecs::Entity, Option<StructureUse>, f32)> = None;
    for (e, (s, tf)) in world.query::<(&Structure, &Transform)>().iter() {
        let Some(t) = ray_hits_box(eye, dir, tf) else { continue };
        if t <= reach && first.map_or(true, |f| t < f.2) {
            first = Some((e, use_of(s), t));
        }
    }
    first.and_then(|(e, u, _)| u.map(|u| (e, u)))
}

/// Give every finished structure that has no uid (0) the next free one.
/// Uids already given, including ones restored from a save, are kept.
pub fn assign_uids(world: &mut hecs::World) {
    let mut next = world
        .query::<&Structure>()
        .iter()
        .map(|(_e, s)| s.uid)
        .max()
        .unwrap_or(0);
    for (_e, s) in world.query_mut::<&mut Structure>() {
        if s.uid == 0 {
            next += 1;
            s.uid = next;
        }
    }
}

/// The organize-layer container path a built store's contents are filed
/// under. Keyed by uid rather than by position in the places tree, so a
/// second chest (or a change to the seeded places) never moves what is in
/// the first. Never numeric, so it cannot collide with a tree index path.
pub fn storage_path(uid: u32) -> String {
    format!("built:{uid}")
}

/// Every finished storage structure as (container path, display name), in
/// uid order. Repeats of a name are numbered so two chests read apart.
pub fn built_stores(world: &hecs::World, registry: Option<&BlueprintRegistry>) -> Vec<(String, String)> {
    let mut stores: Vec<(u32, String)> = world
        .query::<&Structure>()
        .iter()
        .filter(|(_e, s)| s.uid != 0 && use_of(s) == Some(StructureUse::Store))
        .map(|(_e, s)| (s.uid, display_name(s, registry)))
        .collect();
    stores.sort_by_key(|(uid, _)| *uid);
    stores
        .iter()
        .enumerate()
        .map(|(i, (uid, name))| {
            let label = if stores.iter().filter(|(_, n)| n == name).count() > 1 {
                let nth = stores[..=i].iter().filter(|(_, n)| n == name).count();
                format!("{name} {nth}")
            } else {
                name.clone()
            };
            (storage_path(*uid), label)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::construction::Construction;
    use glam::Quat;

    fn shipped() -> BlueprintRegistry {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data")
            .join("blueprints")
            .join("basic.ron");
        BlueprintRegistry::from_ron(&std::fs::read(path).unwrap()).unwrap()
    }

    fn built(reg: &BlueprintRegistry, id: &str, at: Vec3, uid: u32) -> (Transform, Structure) {
        let bp = reg.get(id).unwrap_or_else(|| panic!("{id} in basic.ron"));
        (
            Transform { position: at, rotation: Quat::IDENTITY, scale: Vec3::from_array(bp.size) },
            Structure {
                blueprint_id: id.into(),
                health: bp.health,
                max_health: bp.health,
                provides: bp.provides.clone(),
                uid,
            },
        )
    }

    /// The shipped catalog's bed sleeps and its chest stores, read from the
    /// data (`provides`), not from their ids; a wall and a furnace are not
    /// E-usable. Red before `uses` existed: nothing mapped `rest` or
    /// `storage` to anything, and flipping the two match arms makes the
    /// bed and chest assertions fail.
    #[test]
    fn the_shipped_bed_sleeps_and_the_shipped_chest_stores() {
        let reg = shipped();
        let use_for = |id: &str| reg.get(id).and_then(|b| b.provides.as_deref()).and_then(StructureUse::from_provides);
        assert_eq!(use_for("bed"), Some(StructureUse::Sleep));
        assert_eq!(use_for("storage_chest"), Some(StructureUse::Store));
        assert_eq!(use_for("wood_wall"), None, "shelter is not an E use");
        assert_eq!(use_for("furnace"), None, "a furnace works through the station gate");
        let sleepers = reg.blueprints.values().filter(|b| b.provides.as_deref() == Some("rest")).count();
        let stores = reg.blueprints.values().filter(|b| b.provides.as_deref() == Some("storage")).count();
        assert!(sleepers >= 1 && stores >= 1, "the catalog ships somewhere to sleep and somewhere to store");
    }

    /// The crosshair finds the bed you look down at, not one you look past,
    /// not one out of reach, not a chest behind a wall, and not a scaffold.
    /// Red check: picking the nearest USABLE structure instead of the first
    /// structure hit (skipping walls) makes the walled-chest assertion fail.
    #[test]
    fn the_look_ray_finds_the_usable_structure_under_the_crosshair() {
        let reg = shipped();
        let eye = Vec3::new(0.0, 1.7, 0.0);
        let mut world = hecs::World::new();
        world.spawn(built(&reg, "bed", Vec3::new(0.0, 0.0, -2.0), 1));
        // Aim at the middle of the bed's top face (0.6 m up).
        let at_bed = (Vec3::new(0.0, 0.6, -2.0) - eye).normalize();
        assert_eq!(looked_at(&world, eye, at_bed, 5.0).map(|h| h.1), Some(StructureUse::Sleep));
        assert!(looked_at(&world, eye, Vec3::NEG_Z, 5.0).is_none(), "looking level passes over a 0.6 m bed");
        assert!(looked_at(&world, eye, at_bed, 1.0).is_none(), "out of reach");

        // A chest straight ahead with a wall between: the wall is hit first.
        let mut walled = hecs::World::new();
        walled.spawn(built(&reg, "storage_chest", Vec3::new(0.0, 0.0, -4.0), 1));
        let at_chest = (Vec3::new(0.0, 0.4, -4.0) - eye).normalize();
        assert_eq!(looked_at(&walled, eye, at_chest, 6.0).map(|h| h.1), Some(StructureUse::Store));
        walled.spawn(built(&reg, "wood_wall", Vec3::new(0.0, 0.0, -2.0), 2));
        assert!(looked_at(&walled, eye, at_chest, 6.0).is_none(), "a wall hides the chest behind it");

        // A bed still going up is not a bed yet.
        let mut scaffold = hecs::World::new();
        scaffold.spawn((
            Transform { position: Vec3::new(0.0, 0.0, -2.0), rotation: Quat::IDENTITY, scale: Vec3::new(1.0, 0.6, 2.0) },
            Construction { blueprint_id: "bed".into(), progress: 1.0, build_time: 4.0, builder_key: None },
        ));
        assert!(looked_at(&scaffold, eye, at_bed, 5.0).is_none());
    }

    /// Uids are handed out once and kept: a restored uid is never renumbered
    /// and the new ones never collide with it. Red check: without the
    /// `if s.uid == 0` guard the restored 5 is renumbered and this fails.
    #[test]
    fn uids_are_assigned_once_and_never_collide() {
        let reg = shipped();
        let mut world = hecs::World::new();
        let a = world.spawn(built(&reg, "storage_chest", Vec3::ZERO, 0));
        let b = world.spawn(built(&reg, "storage_chest", Vec3::X, 5));
        let c = world.spawn(built(&reg, "bed", Vec3::Z, 0));
        assign_uids(&mut world);
        let uid = |w: &hecs::World, e| w.get::<&Structure>(e).unwrap().uid;
        let (ua, ub, uc) = (uid(&world, a), uid(&world, b), uid(&world, c));
        assert_eq!(ub, 5, "a restored uid is kept");
        assert!(ua > 5 && uc > 5 && ua != uc, "new uids are fresh: {ua} {uc}");
        assign_uids(&mut world);
        assert_eq!((uid(&world, a), uid(&world, b), uid(&world, c)), (ua, ub, uc), "assigning again changes nothing");
    }

    /// Built stores are addressed by uid, in uid order, and two of the same
    /// kind are numbered; a bed is not a store. Red check: returning the
    /// bare name for repeats leaves two chests labelled alike.
    #[test]
    fn built_stores_are_addressed_by_uid_and_numbered() {
        let reg = shipped();
        let mut world = hecs::World::new();
        world.spawn(built(&reg, "storage_chest", Vec3::ZERO, 7));
        world.spawn(built(&reg, "storage_chest", Vec3::X, 3));
        world.spawn(built(&reg, "bed", Vec3::Z, 4));
        let stores = built_stores(&world, Some(&reg));
        assert_eq!(
            stores,
            vec![
                ("built:3".to_string(), "Storage Chest 1".to_string()),
                ("built:7".to_string(), "Storage Chest 2".to_string()),
            ]
        );
    }
}

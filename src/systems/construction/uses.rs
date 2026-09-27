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
//! `shelter` (walls, roofs) is not an E use: it works by standing in it
//! (2026-09-27). [`shelter_at`] says whether the player is under a built roof
//! with walls around them, and `engine::survival_env` feeds that to the body
//! heat model as `EnvironmentContext::sheltered`: still air and nothing
//! falling. Like `rest` and `storage`, which pieces shelter is data (any
//! blueprint with `provides: Some("shelter")`), and which of them is the roof
//! is where it stands (overhead), not its id or category.

use super::{placement, BlueprintRegistry, Structure};
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

// -- Shelter ---------------------------------------------------------------------

/// The `provides` a built piece carries when it keeps weather off (walls, roofs).
pub const SHELTER: &str = "shelter";
/// Walled sides needed, with a roof overhead, to count as sheltered: three of
/// the four. A roof alone keeps the rain off but not the wind; walls on three
/// sides stop the wind from most of the compass and leave the fourth open as
/// a way in, which is how a lean-to or a three-sided field shelter is built
/// (doors cannot be set into a wall yet, so a fully closed room would have no
/// door). A GAME CHOICE: the weather's wind has a direction, and a later rule
/// could ask whether the open side faces into it.
pub const SHELTER_MIN_WALLS: u8 = 3;
/// Where the side rays run from, metres above the feet: chest height, so a
/// wall counts when it stands between the wind and the body.
const SHELTER_CHEST_M: f32 = 1.0;
/// A roof's underside must clear the head: at least this far above the chest
/// (1.8 m above the feet)...
const SHELTER_HEADROOM_M: f32 = 0.8;
/// ...and no farther above it than this.
const SHELTER_ROOF_REACH_M: f32 = 8.0;
/// A wall counts for a side when it stands inside the roof's footprint or no
/// more than this far outside its edge (a wall just outside the roof line
/// still keeps the wind off what is under it).
const SHELTER_EAVE_M: f32 = 0.5;

/// What the built pieces around a spot do against the weather.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShelterCheck {
    /// A finished shelter piece is overhead.
    pub roofed: bool,
    /// How many of the four sides (east, west, north, south) have a finished
    /// shelter piece within the roof's reach. 0 without a roof.
    pub walled_sides: u8,
}

impl ShelterCheck {
    /// Under a roof with enough walls: the wind and the rain do not reach
    /// the body.
    pub fn sheltered(&self) -> bool {
        self.roofed && self.walled_sides >= SHELTER_MIN_WALLS
    }

    /// One line for the HUD: "Sheltered", or under a roof with walls missing
    /// that the rain is off and how many walls there are, or empty in the
    /// open.
    pub fn note(&self) -> String {
        if self.sheltered() {
            "Sheltered".to_string()
        } else if self.roofed {
            format!("Out of the rain, {} of {} walls", self.walled_sides, SHELTER_MIN_WALLS)
        } else {
            String::new()
        }
    }
}

/// Is a person standing with their feet at `feet` under a finished roof with
/// walls around them? The roof is the nearest finished shelter piece
/// straight overhead (clearing the head, within 8 m). Each side is walled
/// when a level ray at chest height, east, west, north or south, meets
/// another finished shelter piece before it is more than half a metre past
/// the roof's edge. Scaffolds shelter nothing. The boxes are the ones the
/// renderer draws (`ray_hits_box`).
pub fn shelter_at(world: &hecs::World, feet: Vec3) -> ShelterCheck {
    let chest = feet + Vec3::Y * SHELTER_CHEST_M;
    let pieces: Vec<(hecs::Entity, Transform)> = world
        .query::<(&Structure, &Transform)>()
        .iter()
        .filter(|(_e, (s, _))| s.provides.as_deref() == Some(SHELTER))
        .map(|(e, (_, tf))| (e, tf.clone()))
        .collect();
    let roof = pieces
        .iter()
        .filter_map(|(e, tf)| {
            let t = ray_hits_box(chest, Vec3::Y, tf)?;
            (SHELTER_HEADROOM_M..=SHELTER_ROOF_REACH_M).contains(&t).then_some((*e, tf, t))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2));
    let Some((roof_e, roof_tf, _)) = roof else { return ShelterCheck::default() };
    let (lo, hi) = placement::world_aabb(roof_tf);
    let sides = [
        (Vec3::X, hi.x - chest.x),
        (Vec3::NEG_X, chest.x - lo.x),
        (Vec3::Z, hi.z - chest.z),
        (Vec3::NEG_Z, chest.z - lo.z),
    ];
    let walled = sides
        .iter()
        .filter(|(dir, to_edge)| {
            let reach = to_edge.max(0.0) + SHELTER_EAVE_M;
            // A hit at 0 is a wall the person is standing inside (nothing
            // stops walking through built pieces yet): it shelters no side.
            pieces
                .iter()
                .any(|(e, tf)| *e != roof_e && ray_hits_box(chest, *dir, tf).is_some_and(|t| t > 1e-4 && t <= reach))
        })
        .count() as u8;
    ShelterCheck { roofed: true, walled_sides: walled }
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

    /// Place a finished piece the way the ConstructionSystem does
    /// (`placement::placement_pose`), so a roof lands on the walls.
    fn place(world: &mut hecs::World, reg: &BlueprintRegistry, id: &str, x: f32, z: f32, turns: u8) {
        let bp = reg.get(id).unwrap_or_else(|| panic!("{id} in basic.ron"));
        let tf = placement::placement_pose(bp, Vec3::new(x, 0.0, z), turns, world, reg);
        world.spawn((tf, Structure { blueprint_id: id.into(), health: bp.health, max_health: bp.health, provides: bp.provides.clone(), uid: 0 }));
    }

    /// THE SHELTER RULE (2026-09-27). Under a roof on three walls you are
    /// sheltered, and under four; under a roof on two walls you are not (the
    /// note says what is missing), nor under a roof alone, nor in a roofless
    /// room, nor in the open, nor just outside the shelter, nor under a roof
    /// still going up, nor with a wall so far past the roof's edge that it
    /// keeps nothing off you. Red check, run: making `sheltered()` true under
    /// any roof (`self.roofed || ...`) fails the two-walls assertion.
    #[test]
    fn a_roof_on_three_walls_shelters_and_a_roof_alone_does_not() {
        let reg = shipped();
        let feet = Vec3::new(0.0, 0.0, 0.5);
        assert_eq!(shelter_at(&hecs::World::new(), feet), ShelterCheck::default(), "the open");

        // North and west walls, then the roof over them: two sides.
        let mut world = hecs::World::new();
        place(&mut world, &reg, "wood_wall", 0.0, -2.0, 0);
        place(&mut world, &reg, "wood_wall", -2.0, 0.0, 1);
        let mut roofless = hecs::World::new();
        place(&mut roofless, &reg, "wood_wall", 0.0, -2.0, 0);
        place(&mut roofless, &reg, "wood_wall", -2.0, 0.0, 1);
        place(&mut roofless, &reg, "wood_wall", 2.0, 0.0, 1);
        place(&mut roofless, &reg, "wood_wall", 0.0, 2.0, 0);
        assert!(!shelter_at(&roofless, feet).roofed, "four walls and no roof is open to the rain");
        place(&mut world, &reg, "roof", 0.0, 0.0, 0);
        let two = shelter_at(&world, feet);
        assert_eq!(two, ShelterCheck { roofed: true, walled_sides: 2 });
        assert!(!two.sheltered(), "two walls leave the wind in");
        assert_eq!(two.note(), "Out of the rain, 2 of 3 walls");

        // The east wall: three sides, sheltered. The south wall: four.
        place(&mut world, &reg, "wood_wall", 2.0, 0.0, 1);
        let three = shelter_at(&world, feet);
        assert!(three.sheltered() && three.walled_sides == 3, "{three:?}");
        assert_eq!(three.note(), "Sheltered");
        place(&mut world, &reg, "wood_wall", 0.0, 2.0, 0);
        assert_eq!(shelter_at(&world, feet).walled_sides, 4);
        // Anywhere under it, and not a step outside it.
        assert!(shelter_at(&world, Vec3::new(1.5, 0.0, -1.5)).sheltered());
        assert_eq!(shelter_at(&world, Vec3::new(5.0, 0.0, 0.0)), ShelterCheck::default(), "outside the east wall");

        // A roof alone (on posts nothing here builds): rain off, wind in.
        let mut alone = hecs::World::new();
        let roof = reg.get("roof").unwrap();
        alone.spawn((
            Transform { position: Vec3::new(0.0, 3.0, 0.0), rotation: Quat::IDENTITY, scale: Vec3::from_array(roof.size) },
            Structure { blueprint_id: "roof".into(), health: 1.0, max_health: 1.0, provides: roof.provides.clone(), uid: 0 },
        ));
        let a = shelter_at(&alone, feet);
        assert!(a.roofed && a.walled_sides == 0 && !a.sheltered(), "{a:?}");
        // Walls well beyond the roof's edge keep nothing off the person under it.
        for (x, z, t) in [(0.0, -6.0, 0), (-6.0, 0.0, 1), (6.0, 0.0, 1)] {
            place(&mut alone, &reg, "wood_wall", x, z, t);
        }
        assert_eq!(shelter_at(&alone, feet).walled_sides, 0, "walls 4 m past the eaves");

        // A roof still going up keeps nothing off.
        let mut scaffold = hecs::World::new();
        for (x, z, t) in [(0.0, -2.0, 0), (-2.0, 0.0, 1), (2.0, 0.0, 1)] {
            place(&mut scaffold, &reg, "wood_wall", x, z, t);
        }
        scaffold.spawn((
            Transform { position: Vec3::new(0.0, 3.0, 0.0), rotation: Quat::IDENTITY, scale: Vec3::from_array(roof.size) },
            Construction { blueprint_id: "roof".into(), progress: 1.0, build_time: 6.0, builder_key: None },
        ));
        assert!(!shelter_at(&scaffold, feet).roofed, "a scaffold is not a roof yet");
    }
}

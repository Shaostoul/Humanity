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
//!
//! The look ray and the shelter test run in ONE FRAME, like placement: the
//! home frame aboard, or the build site the player stands in on a planet
//! (`site::PlanetSite`), with the player's eye and look direction given in
//! that frame's coordinates. Before 2026-09-27 both took the raw camera
//! position against every piece, which off the ship pointed at nothing real
//! (BUG-102).

use super::site::{in_frame, PlanetSite};
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
/// within `reach` metres: the FIRST structure in `frame` the look ray meets,
/// so a wall between you and a chest hides the chest. `eye` and `dir` are in
/// that frame. Scaffolds (`Construction`) are not structures yet and are
/// never usable.
pub fn looked_at(
    world: &hecs::World,
    eye: Vec3,
    dir: Vec3,
    reach: f32,
    frame: Option<&PlanetSite>,
) -> Option<(hecs::Entity, StructureUse)> {
    let e = first_in_view(world, eye, dir, reach, frame)?;
    let u = world.get::<&Structure>(e).ok().and_then(|s| use_of(&s))?;
    Some((e, u))
}

/// The FIRST finished structure in `frame` the look ray meets within
/// `reach` metres, whatever it is: the piece Take down removes, and the one
/// [`looked_at`] asks the use of. Scaffolds (`Construction`) are not
/// structures and are never met.
pub fn first_in_view(world: &hecs::World, eye: Vec3, dir: Vec3, reach: f32, frame: Option<&PlanetSite>) -> Option<hecs::Entity> {
    let dir = dir.normalize_or_zero();
    if dir == Vec3::ZERO {
        return None;
    }
    let mut first: Option<(hecs::Entity, f32)> = None;
    for (e, (_s, tf, site)) in world.query::<(&Structure, &Transform, Option<&PlanetSite>)>().iter() {
        if !in_frame(site, frame) {
            continue;
        }
        let Some(t) = ray_hits_box(eye, dir, tf) else { continue };
        if t <= reach && first.map_or(true, |f| t < f.1) {
            first = Some((e, t));
        }
    }
    first.map(|(e, _)| e)
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
/// Walled sides a finished shelter has, with a roof overhead: three of the
/// four, leaving the fourth open as a way in, which is how a lean-to or a
/// three-sided field shelter is built (doors cannot be set into a wall yet, so
/// a fully closed room would have no door). It is the HUD's build goal. What
/// the WIND does is decided by which sides are walled (`ShelterCheck::wind_share`),
/// so three walls keep the wind off only with the open side turned away from
/// it: the survival manuals' "back to the wind" (2026-09-28).
pub const SHELTER_MIN_WALLS: u8 = 3;
/// The four sides a shelter check looks along, in the check's frame (on a
/// planet the build site's: +X east, -Z north, `site::tangent_basis`). Bit i
/// of `ShelterCheck::walls` is side i.
pub const SHELTER_SIDES: [Vec3; 4] = [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z];
/// Where the side rays run from, metres above the feet: chest height, so a
/// wall counts when it stands between the wind and the body.
const SHELTER_CHEST_M: f32 = 1.0;
/// A roof's underside must clear the head: at least this far above the chest
/// (1.8 m above the feet)...
const SHELTER_HEADROOM_M: f32 = 0.8;
/// ...and no farther above it than this.
const SHELTER_ROOF_REACH_M: f32 = 8.0;
/// A wall counts for a side when it stands inside the covered area or no
/// more than this far outside its edge (a wall just outside the roof line
/// still keeps the wind off what is under it).
const SHELTER_EAVE_M: f32 = 0.5;
/// Roofs that touch, overlap, or leave a gap narrower than this (m) are one
/// covered area: a hall under four roof tiles is one roof, not four.
const SHELTER_JOIN_M: f32 = 0.05;

/// What the built pieces around a spot do against the weather.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShelterCheck {
    /// A finished shelter piece is overhead.
    pub roofed: bool,
    /// Which of the four [`SHELTER_SIDES`] have a finished shelter piece
    /// within the roof's reach, bit i for side i. 0 without a roof.
    pub walls: u8,
}

impl ShelterCheck {
    /// How many of the four sides are walled.
    pub fn walled_sides(&self) -> u8 {
        self.walls.count_ones() as u8
    }

    /// Built as a shelter: a roof on at least [`SHELTER_MIN_WALLS`] walls.
    /// Whether the wind gets in as well depends on where it comes from:
    /// [`Self::wind_share`].
    pub fn sheltered(&self) -> bool {
        self.roofed && self.walled_sides() >= SHELTER_MIN_WALLS
    }

    /// The share of the wind that reaches a person under this roof, 0 to 1,
    /// when it blows FROM `upwind` (a horizontal direction in the check's
    /// frame; zero for calm air). Out in the open, or with no roof, all of
    /// it. Under a roof, what comes in through the open sides that face into
    /// the wind, each side letting in the cosine of the wind's angle to it:
    /// a wall on the windward side stops it, the lee side can stand open, and
    /// a wind straight into the open side of a three-walled shelter comes in
    /// in full. The cosine split is A GAME CHOICE (how much of an oblique
    /// wind a half-open corner lets through is a matter for airflow models
    /// this does not run); the direction rule is the survival manuals'.
    pub fn wind_share(&self, upwind: Vec3) -> f32 {
        if !self.roofed {
            return 1.0;
        }
        let u = Vec3::new(upwind.x, 0.0, upwind.z).normalize_or_zero();
        SHELTER_SIDES
            .iter()
            .enumerate()
            .filter(|(i, _)| self.walls & (1 << i) == 0)
            .map(|(_, side)| side.dot(u).max(0.0))
            .sum::<f32>()
            .min(1.0)
    }

    /// Out of the rain AND the wind blowing from `upwind`: what the body
    /// heat model treats as still air (`body_heat::Exposure::from_context`).
    pub fn out_of_the_wind(&self, upwind: Vec3) -> bool {
        self.roofed && self.wind_share(upwind) <= 1.0e-3
    }

    /// One line for the HUD, for the wind blowing from `upwind`: "Sheltered"
    /// under a finished shelter the wind does not reach, "Out of the rain and
    /// the wind" under a roof whose walls happen to face it, how much of the
    /// wind gets in when an open side faces it, and empty in the open.
    pub fn note(&self, upwind: Vec3) -> String {
        if !self.roofed {
            String::new()
        } else if self.out_of_the_wind(upwind) {
            if self.sheltered() {
                "Sheltered".to_string()
            } else {
                "Out of the rain and the wind".to_string()
            }
        } else {
            format!("Out of the rain; {:.0}% of the wind gets in", self.wind_share(upwind) * 100.0)
        }
    }
}

/// Is a person standing with their feet at `feet` (in `frame`) under a
/// finished roof with walls around them?
///
/// The roof: a finished shelter piece in `frame` straight overhead,
/// clearing the head and within 8 m. The COVERED AREA is that roof and every
/// roof joined to it: roofs are the shelter pieces whose underside is at
/// roof height over the chest, and two roofs are joined when they touch or
/// overlap (a gap under [`SHELTER_JOIN_M`] counts as touching), so a hall
/// under four roof tiles is one covered area (review of the shelter commit:
/// the old test reached only half a metre past the ONE tile overhead, so no
/// building bigger than a tile ever counted). Each side is walled when a
/// level ray at chest height, east, west, north or south, meets another
/// finished shelter piece before it is more than half a metre past where the
/// covered area ends along that ray ([`covered_run`]). Scaffolds shelter
/// nothing. The boxes are the ones the renderer draws (`ray_hits_box`).
pub fn shelter_at(world: &hecs::World, feet: Vec3, frame: Option<&PlanetSite>) -> ShelterCheck {
    let chest = feet + Vec3::Y * SHELTER_CHEST_M;
    let pieces: Vec<(hecs::Entity, Transform)> = world
        .query::<(&Structure, &Transform, Option<&PlanetSite>)>()
        .iter()
        .filter(|(_e, (s, _, site))| s.provides.as_deref() == Some(SHELTER) && in_frame(*site, frame))
        .map(|(e, (_, tf, _))| (e, tf.clone()))
        .collect();
    let overhead = pieces.iter().any(|(_e, tf)| {
        ray_hits_box(chest, Vec3::Y, tf).is_some_and(|t| (SHELTER_HEADROOM_M..=SHELTER_ROOF_REACH_M).contains(&t))
    });
    if !overhead {
        return ShelterCheck::default();
    }
    // Every piece at roof height over this chest, as its box.
    let roofs: Vec<(hecs::Entity, (Vec3, Vec3))> = pieces
        .iter()
        .map(|(e, tf)| (*e, placement::world_aabb(tf)))
        .filter(|(_e, b)| (SHELTER_HEADROOM_M..=SHELTER_ROOF_REACH_M).contains(&(b.0.y - chest.y)))
        .collect();
    let boxes: Vec<(Vec3, Vec3)> = roofs.iter().map(|(_e, b)| *b).collect();
    let mut walls = 0u8;
    for (i, dir) in SHELTER_SIDES.iter().enumerate() {
        let reach = covered_run(chest, *dir, &boxes) + SHELTER_EAVE_M;
        // A hit at 0 is a wall the person is standing inside (nothing
        // stops walking through built pieces yet): it shelters no side.
        let walled = pieces.iter().any(|(e, tf)| {
            !roofs.iter().any(|(r, _)| r == e) && ray_hits_box(chest, *dir, tf).is_some_and(|t| t > 1e-4 && t <= reach)
        });
        if walled {
            walls |= 1 << i;
        }
    }
    ShelterCheck { roofed: true, walls }
}

/// How far along the level ray from `chest` in the axis direction `dir` the
/// covered area runs: from the roof boxes over the chest, on through every
/// roof box that touches the one before it along the ray (within
/// [`SHELTER_JOIN_M`]), to where the last of them ends. 0 when nothing
/// covers the chest.
pub fn covered_run(chest: Vec3, dir: Vec3, roofs: &[(Vec3, Vec3)]) -> f32 {
    let covers = |b: &(Vec3, Vec3), p: Vec3| {
        p.x >= b.0.x - SHELTER_JOIN_M
            && p.x <= b.1.x + SHELTER_JOIN_M
            && p.z >= b.0.z - SHELTER_JOIN_M
            && p.z <= b.1.z + SHELTER_JOIN_M
    };
    // Where the ray leaves a box's footprint, measured from the chest.
    let exit = |b: &(Vec3, Vec3)| -> f32 {
        if dir.x > 0.5 {
            b.1.x - chest.x
        } else if dir.x < -0.5 {
            chest.x - b.0.x
        } else if dir.z > 0.5 {
            b.1.z - chest.z
        } else {
            chest.z - b.0.z
        }
    };
    let mut t = 0.0_f32;
    // Each pass moves on to a roof that ends farther along; there are only
    // so many roofs, so this ends.
    for _ in 0..=roofs.len() {
        let p = chest + dir * t;
        let next = roofs.iter().filter(|b| covers(b, p)).map(exit).fold(f32::NEG_INFINITY, f32::max);
        if next > t + 1e-4 {
            t = next;
        } else {
            break;
        }
    }
    t
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
        assert_eq!(looked_at(&world, eye, at_bed, 5.0, None).map(|h| h.1), Some(StructureUse::Sleep));
        assert!(looked_at(&world, eye, Vec3::NEG_Z, 5.0, None).is_none(), "looking level passes over a 0.6 m bed");
        assert!(looked_at(&world, eye, at_bed, 1.0, None).is_none(), "out of reach");

        // A chest straight ahead with a wall between: the wall is hit first.
        let mut walled = hecs::World::new();
        walled.spawn(built(&reg, "storage_chest", Vec3::new(0.0, 0.0, -4.0), 1));
        let at_chest = (Vec3::new(0.0, 0.4, -4.0) - eye).normalize();
        assert_eq!(looked_at(&walled, eye, at_chest, 6.0, None).map(|h| h.1), Some(StructureUse::Store));
        walled.spawn(built(&reg, "wood_wall", Vec3::new(0.0, 0.0, -2.0), 2));
        assert!(looked_at(&walled, eye, at_chest, 6.0, None).is_none(), "a wall hides the chest behind it");

        // A bed still going up is not a bed yet.
        let mut scaffold = hecs::World::new();
        scaffold.spawn((
            Transform { position: Vec3::new(0.0, 0.0, -2.0), rotation: Quat::IDENTITY, scale: Vec3::new(1.0, 0.6, 2.0) },
            Construction { blueprint_id: "bed".into(), progress: 1.0, build_time: 4.0, builder_key: None },
        ));
        assert!(looked_at(&scaffold, eye, at_bed, 5.0, None).is_none());
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
        let tf = placement::placement_pose(bp, Vec3::new(x, 0.0, z), turns, world, reg, None);
        world.spawn((tf, Structure { blueprint_id: id.into(), health: bp.health, max_health: bp.health, provides: bp.provides.clone(), uid: 0 }));
    }

    /// BACK TO THE WIND (2026-09-28). The same three-walled shelter, open to
    /// the south: a north wind meets a wall and none of it reaches the person
    /// inside; a south wind blows straight in the open side, all of it; a
    /// wind from the south-east comes in at the cosine of its angle to the
    /// open side. Four walls stop every wind, and in the open all of it
    /// reaches you. Red check, run: counting the open sides without the
    /// direction (`share = open sides / 4`) fails the north-wind assertion.
    #[test]
    fn three_walls_shelter_only_with_their_back_to_the_wind() {
        let reg = shipped();
        let feet = Vec3::new(0.0, 0.0, 0.5);
        let mut world = hecs::World::new();
        for (x, z, t) in [(0.0, -2.0, 0), (-2.0, 0.0, 1), (2.0, 0.0, 1)] {
            place(&mut world, &reg, "wood_wall", x, z, t);
        }
        place(&mut world, &reg, "roof", 0.0, 0.0, 0);
        let s = shelter_at(&world, feet, None);
        assert_eq!(s.walls, 0b1011, "east, west and north walled, south open: {s:?}");
        let north = Vec3::NEG_Z;
        let south = Vec3::Z;
        let south_east = Vec3::new(1.0, 0.0, 1.0);
        assert_eq!(s.wind_share(north), 0.0, "a north wind meets the north wall");
        assert!(s.out_of_the_wind(north) && s.note(north) == "Sheltered");
        assert!((s.wind_share(south) - 1.0).abs() < 1e-6, "a south wind comes in the open side");
        assert_eq!(s.note(south), "Out of the rain; 100% of the wind gets in");
        assert!((s.wind_share(south_east) - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5, "{}", s.wind_share(south_east));
        assert_eq!(s.wind_share(Vec3::ZERO), 0.0, "calm air");
        let closed = ShelterCheck { roofed: true, walls: 0b1111 };
        for u in [north, south, south_east, Vec3::X, Vec3::NEG_X] {
            assert_eq!(closed.wind_share(u), 0.0, "four walls, wind from {u}");
        }
        assert_eq!(ShelterCheck::default().wind_share(north), 1.0, "the open");
        assert_eq!(ShelterCheck::default().note(north), "");
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
        assert_eq!(shelter_at(&hecs::World::new(), feet, None), ShelterCheck::default(), "the open");

        // North and west walls, then the roof over them: two sides.
        let mut world = hecs::World::new();
        place(&mut world, &reg, "wood_wall", 0.0, -2.0, 0);
        place(&mut world, &reg, "wood_wall", -2.0, 0.0, 1);
        let mut roofless = hecs::World::new();
        place(&mut roofless, &reg, "wood_wall", 0.0, -2.0, 0);
        place(&mut roofless, &reg, "wood_wall", -2.0, 0.0, 1);
        place(&mut roofless, &reg, "wood_wall", 2.0, 0.0, 1);
        place(&mut roofless, &reg, "wood_wall", 0.0, 2.0, 0);
        assert!(!shelter_at(&roofless, feet, None).roofed, "four walls and no roof is open to the rain");
        place(&mut world, &reg, "roof", 0.0, 0.0, 0);
        let two = shelter_at(&world, feet, None);
        // West (bit 1) and north (bit 3).
        assert_eq!(two, ShelterCheck { roofed: true, walls: 0b1010 });
        assert!(!two.sheltered(), "two walls are not yet a shelter");
        assert_eq!(two.note(Vec3::X), "Out of the rain; 100% of the wind gets in", "an east wind");
        assert_eq!(two.note(Vec3::NEG_Z), "Out of the rain and the wind", "a north wind meets the north wall");

        // The east wall: three sides, sheltered. The south wall: four.
        place(&mut world, &reg, "wood_wall", 2.0, 0.0, 1);
        let three = shelter_at(&world, feet, None);
        assert!(three.sheltered() && three.walled_sides() == 3, "{three:?}");
        assert_eq!(three.note(Vec3::NEG_Z), "Sheltered", "back to a north wind");
        place(&mut world, &reg, "wood_wall", 0.0, 2.0, 0);
        assert_eq!(shelter_at(&world, feet, None).walled_sides(), 4);
        // Anywhere under it, and not a step outside it.
        assert!(shelter_at(&world, Vec3::new(1.5, 0.0, -1.5), None).sheltered());
        assert_eq!(shelter_at(&world, Vec3::new(5.0, 0.0, 0.0), None), ShelterCheck::default(), "outside the east wall");

        // A roof alone (on posts nothing here builds): rain off, wind in.
        let mut alone = hecs::World::new();
        let roof = reg.get("roof").unwrap();
        alone.spawn((
            Transform { position: Vec3::new(0.0, 3.0, 0.0), rotation: Quat::IDENTITY, scale: Vec3::from_array(roof.size) },
            Structure { blueprint_id: "roof".into(), health: 1.0, max_health: 1.0, provides: roof.provides.clone(), uid: 0 },
        ));
        let a = shelter_at(&alone, feet, None);
        assert!(a.roofed && a.walled_sides() == 0 && !a.sheltered(), "{a:?}");
        // Walls well beyond the roof's edge keep nothing off the person under it.
        for (x, z, t) in [(0.0, -6.0, 0), (-6.0, 0.0, 1), (6.0, 0.0, 1)] {
            place(&mut alone, &reg, "wood_wall", x, z, t);
        }
        assert_eq!(shelter_at(&alone, feet, None).walled_sides(), 0, "walls 4 m past the eaves");

        // A roof still going up keeps nothing off.
        let mut scaffold = hecs::World::new();
        for (x, z, t) in [(0.0, -2.0, 0), (-2.0, 0.0, 1), (2.0, 0.0, 1)] {
            place(&mut scaffold, &reg, "wood_wall", x, z, t);
        }
        scaffold.spawn((
            Transform { position: Vec3::new(0.0, 3.0, 0.0), rotation: Quat::IDENTITY, scale: Vec3::from_array(roof.size) },
            Construction { blueprint_id: "roof".into(), progress: 1.0, build_time: 6.0, builder_key: None },
        ));
        assert!(!shelter_at(&scaffold, feet, None).roofed, "a scaffold is not a roof yet");
    }

    /// An 8 x 8 m hall: eight walls round the outside (two per side, the
    /// east and west ones turned) and four roof tiles laid on them, placed
    /// the way the ghost places them. Every point inside it is sheltered,
    /// on four sides, including the points under the seams where the tiles
    /// meet; a step outside it is not. Red check, run: stopping
    /// `covered_run` after its first pass (the roof over the chest and no
    /// further: the old test, which looked for walls only half a metre past
    /// the ONE tile overhead, review of the shelter commit) leaves a point
    /// in the middle of a tile with only that tile's two outer walls, and
    /// the first assertion fails.
    #[test]
    fn an_8m_hall_of_four_roofs_on_eight_walls_shelters_every_interior_point() {
        let reg = shipped();
        let mut world = hecs::World::new();
        for x in [-2.0, 2.0] {
            place(&mut world, &reg, "wood_wall", x, -4.0, 0);
            place(&mut world, &reg, "wood_wall", x, 4.0, 0);
        }
        for z in [-2.0, 2.0] {
            place(&mut world, &reg, "wood_wall", -4.0, z, 1);
            place(&mut world, &reg, "wood_wall", 4.0, z, 1);
        }
        for (x, z) in [(-2.0, -2.0), (2.0, -2.0), (-2.0, 2.0), (2.0, 2.0)] {
            place(&mut world, &reg, "roof", x, z, 0);
        }
        let roof_y: Vec<f32> = world
            .query::<(&Structure, &Transform)>()
            .iter()
            .filter(|(_e, (s, _))| s.blueprint_id == "roof")
            .map(|(_e, (_, t))| t.position.y)
            .collect();
        assert!(roof_y.iter().all(|y| (y - 3.0).abs() < 1e-4), "every tile rests on the walls: {roof_y:?}");
        let mut checked = 0;
        for i in 0..15 {
            for j in 0..15 {
                let feet = Vec3::new(-3.5 + 0.5 * i as f32, 0.0, -3.5 + 0.5 * j as f32);
                let s = shelter_at(&world, feet, None);
                assert_eq!(s, ShelterCheck { roofed: true, walls: 0b1111 }, "inside the hall at {feet}");
                checked += 1;
            }
        }
        assert_eq!(checked, 225);
        assert_eq!(shelter_at(&world, Vec3::new(6.0, 0.0, 0.0), None), ShelterCheck::default(), "outside");

        // The covered area follows the tiles that touch: take one tile away
        // and the corner under it is open to the sky again, while the far
        // corner is still under three touching tiles and four walls.
        let gone = world
            .query::<(&Structure, &Transform)>()
            .iter()
            .find(|(_e, (s, t))| s.blueprint_id == "roof" && t.position.x > 0.0 && t.position.z > 0.0)
            .map(|(e, _)| e)
            .unwrap();
        world.despawn(gone).unwrap();
        assert!(!shelter_at(&world, Vec3::new(2.0, 0.0, 2.0), None).roofed, "under the missing tile");
        assert!(shelter_at(&world, Vec3::new(-2.0, 0.0, -2.0), None).sheltered(), "the far corner");
    }

    /// The shelter and the look ray see only the pieces of the frame they are
    /// asked about: a room built at a planet site shelters a person standing
    /// in that site, and nobody at the same numbers in the home frame; a bed
    /// in the site is looked at in the site only. Red check, run: dropping
    /// the `in_frame` filter from `shelter_at` makes the home-frame person
    /// sheltered by the planet's roof and the second assertion fails.
    #[test]
    fn shelter_and_look_ray_stay_in_their_frame() {
        use super::super::site::PlanetSite;
        let reg = shipped();
        let site = PlanetSite { body: "earth".into(), origin: glam::DVec3::new(0.0, 6_371_000.0, 0.0) };
        let mut world = hecs::World::new();
        for (id, x, z, t) in [("wood_wall", 0.0, -2.0, 0), ("wood_wall", -2.0, 0.0, 1), ("wood_wall", 2.0, 0.0, 1), ("roof", 0.0, 0.0, 0), ("bed", 0.0, 1.0, 0)] {
            let bp = reg.get(id).unwrap();
            let tf = placement::placement_pose(bp, Vec3::new(x, 0.0, z), t, &world, &reg, Some(&site));
            world.spawn((tf, Structure { blueprint_id: id.into(), health: 1.0, max_health: 1.0, provides: bp.provides.clone(), uid: 0 }, site.clone()));
        }
        let feet = Vec3::new(0.0, 0.0, 0.5);
        assert!(shelter_at(&world, feet, Some(&site)).sheltered(), "in the site");
        assert_eq!(shelter_at(&world, feet, None), ShelterCheck::default(), "the home frame has no roof here");
        let eye = Vec3::new(0.0, 1.7, -1.0);
        let at_bed = (Vec3::new(0.0, 0.6, 1.0) - eye).normalize();
        assert_eq!(looked_at(&world, eye, at_bed, 5.0, Some(&site)).map(|h| h.1), Some(StructureUse::Sleep));
        assert!(looked_at(&world, eye, at_bed, 5.0, None).is_none());
    }
}

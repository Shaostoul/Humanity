//! Where a blueprint lands when the player places it (2026-09-27).
//!
//! One function, [`placement_pose`], answers it for both the ghost preview
//! (`engine::build_place`, the see-through piece that follows the crosshair)
//! and the ConstructionSystem that builds it, so the piece is built exactly
//! where the ghost stood:
//!
//! - x and z snap to the metre grid, from the floor point the player aims at
//!   ([`aim_point`]);
//! - the piece turns in quarter turns about the vertical (R while placing);
//! - y is the floor, or, for a blueprint that says `mount: OnTop`, the top of
//!   the tallest `snap_to` piece its footprint covers: a roof over walls rests
//!   at the walls' height, a wall on a foundation on the foundation.
//!
//! This file used to hold a `PlacementState` and a `PlacementSystem` that no
//! module declared, so they never compiled and nothing used them; the build
//! menu placed everything 4 m ahead, on the floor, unturned. They are gone,
//! and this is the placement that is used.
//!
//! Boxes here are the renderer's: the unit box scaled by the transform, its
//! BOTTOM at `position.y` (`Mesh::box_xyz` is bottom-origin), rotated about
//! that point (the same convention as `uses::ray_hits_box`).

use super::{Blueprint, BlueprintRegistry, Mount, Structure};
use crate::ecs::components::Transform;
use glam::{Mat3, Quat, Vec3};

/// The placement grid, metres. Walls are 4 m long, so a 4 m room lines up.
pub const GRID_M: f32 = 1.0;
/// How far ahead a piece goes when the look ray does not meet the floor
/// (looking level or up): the fixed distance the build menu always used.
pub const DEFAULT_REACH_M: f32 = 4.0;
/// The nearest and farthest a piece can be placed along the floor. Near
/// enough to stand inside a room and aim at its middle; far enough to lay a
/// wall a room's width away.
pub const MIN_REACH_M: f32 = 1.5;
pub const MAX_REACH_M: f32 = 8.0;
/// Two footprints must overlap by more than this (m) to rest one on the
/// other, so a wall that only touches the roof's edge line does not count.
const OVERLAP_EPS_M: f32 = 0.01;

/// A turn of `quarter_turns` times 90 degrees about the vertical.
pub fn quarter_turn(quarter_turns: u8) -> Quat {
    Quat::from_rotation_y(f32::from(quarter_turns % 4) * std::f32::consts::FRAC_PI_2)
}

/// The floor point the player is aiming at, from the eye and the look
/// direction: where the look ray meets the floor, kept between
/// [`MIN_REACH_M`] and [`MAX_REACH_M`] along the ground, or
/// [`DEFAULT_REACH_M`] straight ahead when the ray never meets the floor.
/// The returned y is `floor_y`.
pub fn aim_point(eye: Vec3, forward: Vec3, floor_y: f32) -> Vec3 {
    let flat = Vec3::new(forward.x, 0.0, forward.z);
    let flat = if flat.length_squared() > 1e-8 { flat.normalize() } else { Vec3::NEG_Z };
    let mut reach = DEFAULT_REACH_M;
    if forward.y < -1e-3 {
        let t = (floor_y - eye.y) / forward.y;
        if t > 0.0 {
            let hit = eye + forward * t;
            reach = Vec3::new(hit.x - eye.x, 0.0, hit.z - eye.z).length().clamp(MIN_REACH_M, MAX_REACH_M);
        }
    }
    Vec3::new(eye.x + flat.x * reach, floor_y, eye.z + flat.z * reach)
}

/// The world-space box a structure's transform covers, (min, max). Exact for
/// the quarter turns placement makes; for any other rotation it is the box
/// around the turned box.
pub fn world_aabb(tf: &Transform) -> (Vec3, Vec3) {
    let m = Mat3::from_quat(tf.rotation);
    let half = tf.scale * 0.5;
    let center = tf.position + tf.rotation * Vec3::new(0.0, half.y, 0.0);
    let ext = Vec3::new(
        m.x_axis.x.abs() * half.x + m.y_axis.x.abs() * half.y + m.z_axis.x.abs() * half.z,
        m.x_axis.y.abs() * half.x + m.y_axis.y.abs() * half.y + m.z_axis.y.abs() * half.z,
        m.x_axis.z.abs() * half.x + m.y_axis.z.abs() * half.y + m.z_axis.z.abs() * half.z,
    );
    (center - ext, center + ext)
}

/// Do two boxes overlap on the ground (x and z), by more than a hair?
fn footprints_overlap(a: &(Vec3, Vec3), b: &(Vec3, Vec3)) -> bool {
    let ox = a.1.x.min(b.1.x) - a.0.x.max(b.0.x);
    let oz = a.1.z.min(b.1.z) - a.0.z.max(b.0.z);
    ox > OVERLAP_EPS_M && oz > OVERLAP_EPS_M
}

/// The height `bp` rests at when its box is `candidate` (bottom at the
/// floor): `floor_y` for a floor piece; for `mount: OnTop`, the top of the
/// tallest FINISHED structure whose blueprint category is in `bp.snap_to`
/// and whose footprint overlaps the candidate's, or `floor_y` when none does.
/// Scaffolds still going up hold nothing.
pub fn rest_height(
    bp: &Blueprint,
    candidate: &Transform,
    floor_y: f32,
    world: &hecs::World,
    registry: &BlueprintRegistry,
) -> f32 {
    if bp.mount != Mount::OnTop || bp.snap_to.is_empty() {
        return floor_y;
    }
    let mine = world_aabb(candidate);
    let mut top: Option<f32> = None;
    for (_e, (s, tf)) in world.query::<(&Structure, &Transform)>().iter() {
        let Some(under) = registry.get(&s.blueprint_id) else { continue };
        if !bp.snap_to.iter().any(|c| *c == under.category) {
            continue;
        }
        let theirs = world_aabb(tf);
        if footprints_overlap(&mine, &theirs) {
            top = Some(top.map_or(theirs.1.y, |t| t.max(theirs.1.y)));
        }
    }
    top.unwrap_or(floor_y)
}

/// Where `bp` is built when the player aims at the floor point `at` with
/// `quarter_turns` turns: x and z on the metre grid, turned, scaled to the
/// blueprint's size, and resting at [`rest_height`].
pub fn placement_pose(
    bp: &Blueprint,
    at: Vec3,
    quarter_turns: u8,
    world: &hecs::World,
    registry: &BlueprintRegistry,
) -> Transform {
    let snapped = Vec3::new((at.x / GRID_M).round() * GRID_M, at.y, (at.z / GRID_M).round() * GRID_M);
    let mut tf = Transform { position: snapped, rotation: quarter_turn(quarter_turns), scale: Vec3::from_array(bp.size) };
    tf.position.y = rest_height(bp, &tf, at.y, world, registry);
    tf
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> BlueprintRegistry {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("blueprints").join("basic.ron");
        BlueprintRegistry::from_ron(&std::fs::read(path).unwrap()).unwrap()
    }

    /// Finish a piece in `world` at `at` with `turns`, the way the
    /// ConstructionSystem places it.
    fn build(world: &mut hecs::World, reg: &BlueprintRegistry, id: &str, at: Vec3, turns: u8) -> Transform {
        let bp = reg.get(id).unwrap_or_else(|| panic!("{id} in basic.ron"));
        let tf = placement_pose(bp, at, turns, world, reg);
        world.spawn((
            tf.clone(),
            Structure { blueprint_id: id.into(), health: bp.health, max_health: bp.health, provides: bp.provides.clone(), uid: 0 },
        ));
        tf
    }

    /// A 4 m square room: four walls on the grid around (0, 0), the east and
    /// west ones turned a quarter.
    fn room(world: &mut hecs::World, reg: &BlueprintRegistry, floor: f32) {
        build(world, reg, "wood_wall", Vec3::new(0.0, floor, -2.0), 0);
        build(world, reg, "wood_wall", Vec3::new(0.0, floor, 2.0), 0);
        build(world, reg, "wood_wall", Vec3::new(-2.0, floor, 0.0), 1);
        build(world, reg, "wood_wall", Vec3::new(2.0, floor, 0.0), 1);
    }

    /// A turned wall runs north-south: one quarter turn swaps its 4 m length
    /// onto z. Red check, run: `quarter_turn` returning no turn (what the
    /// build menu did before) leaves it running east-west and the extent
    /// assertion fails.
    #[test]
    fn a_turned_wall_runs_the_other_way() {
        let reg = shipped();
        let mut world = hecs::World::new();
        let tf = build(&mut world, &reg, "wood_wall", Vec3::new(2.2, 0.0, -0.4), 1);
        assert_eq!((tf.position.x, tf.position.z), (2.0, 0.0), "on the metre grid");
        let (lo, hi) = world_aabb(&tf);
        assert!((hi.z - lo.z - 4.0).abs() < 1e-4 && (hi.x - lo.x - 0.2).abs() < 1e-4, "{lo} {hi}");
        let (lo, hi) = world_aabb(&build(&mut world, &reg, "wood_wall", Vec3::new(9.0, 0.0, 9.0), 4));
        assert!((hi.x - lo.x - 4.0).abs() < 1e-4, "four quarter turns is no turn: {lo} {hi}");
    }

    /// A roof aimed into a walled room rests on the walls' tops, 3 m up, and
    /// on walls standing on a foundation, 3.2 m up. A floor piece (a bed) in
    /// the same spot stays on the floor, and a roof with no walls under it
    /// lies on the floor. Red check, run: making `rest_height` return
    /// `floor_y` for every blueprint (the old floor-level placement) puts the
    /// roof at 0 and the first height assertion fails.
    #[test]
    fn a_roof_snaps_to_the_top_of_the_walls_it_covers() {
        let reg = shipped();
        assert_eq!(reg.get("roof").unwrap().mount, Mount::OnTop, "the shipped roof goes on top");
        let mut world = hecs::World::new();
        room(&mut world, &reg, 0.0);
        let roof = build(&mut world, &reg, "roof", Vec3::new(0.3, 0.0, -0.2), 0);
        assert!((roof.position.y - 3.0).abs() < 1e-4, "roof at {}", roof.position.y);
        assert_eq!((roof.position.x, roof.position.z), (0.0, 0.0));
        let bed = placement_pose(reg.get("bed").unwrap(), Vec3::ZERO, 0, &world, &reg);
        assert_eq!(bed.position.y, 0.0, "a bed stands on the floor, not on the roof");

        // Walls on a foundation stand on it, and the roof on them.
        let mut raised = hecs::World::new();
        build(&mut raised, &reg, "wood_foundation", Vec3::ZERO, 0);
        room(&mut raised, &reg, 0.0);
        let wall_y: Vec<f32> = raised
            .query::<(&Structure, &Transform)>()
            .iter()
            .filter(|(_, (s, _))| s.blueprint_id == "wood_wall")
            .map(|(_, (_, t))| t.position.y)
            .collect();
        assert!(wall_y.iter().all(|y| (y - 0.2).abs() < 1e-4), "walls on the foundation: {wall_y:?}");
        let roof = build(&mut raised, &reg, "roof", Vec3::ZERO, 0);
        assert!((roof.position.y - 3.2).abs() < 1e-4, "roof at {}", roof.position.y);

        // Nothing to rest on: the floor.
        let empty = hecs::World::new();
        assert_eq!(placement_pose(reg.get("roof").unwrap(), Vec3::new(0.0, 0.5, 0.0), 0, &empty, &reg).position.y, 0.5);
    }

    /// The aim point: where the look ray meets the floor, clamped along the
    /// ground; straight ahead at 4 m when looking level or up.
    #[test]
    fn the_aim_point_follows_the_look_ray_to_the_floor() {
        let eye = Vec3::new(0.0, 1.7, 0.0);
        let down = Vec3::new(0.0, -1.7, -3.0).normalize();
        let p = aim_point(eye, down, 0.0);
        assert!((p - Vec3::new(0.0, 0.0, -3.0)).length() < 1e-4, "{p}");
        let level = aim_point(eye, Vec3::X, 0.0);
        assert!((level - Vec3::new(DEFAULT_REACH_M, 0.0, 0.0)).length() < 1e-4, "{level}");
        let feet_down = aim_point(eye, Vec3::new(0.0, -1.0, -0.01).normalize(), 0.0);
        assert!((feet_down.z + MIN_REACH_M).abs() < 1e-3, "never inside the player: {feet_down}");
        let far = aim_point(eye, Vec3::new(0.0, -0.01, -1.0).normalize(), 0.0);
        assert!((far.z + MAX_REACH_M).abs() < 1e-3, "{far}");
    }
}

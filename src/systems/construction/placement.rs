//! Where a blueprint lands when the player places it (2026-09-27).
//!
//! One function, [`placement_pose`], answers it for the ghost preview
//! (`engine::build_place`, the see-through piece that follows the crosshair).
//! The ghost's pose is kept on the placing state every frame, and the E press
//! builds exactly that pose (the `BuildRequest` carries it), so the piece is
//! built where the ghost stood:
//!
//! - x and z snap to the metre grid, from the ground point the player aims
//!   at ([`aim_point_on_ground`]);
//! - the piece turns in quarter turns about the vertical (R while placing);
//! - y is the floor, or, for a blueprint that says `mount: OnTop`, the top of
//!   the tallest `snap_to` piece its footprint covers on the player's own
//!   storey: a roof over walls rests at the walls' height, a wall on a
//!   foundation on the foundation, and nothing lands on the storey above or
//!   below ([`STOREY_STEP_M`]).
//!
//! Everything here runs in ONE FRAME at a time: the home frame aboard, or a
//! build site's flat frame on a planet (`site::PlanetSite`, passed as
//! `frame`), and only pieces in that frame are looked at. So "the floor" and
//! "the vertical" are the frame's own Y, which on a planet is the local up.
//!
//! This file used to hold a `PlacementState` and a `PlacementSystem` that no
//! module declared, so they never compiled and nothing used them; the build
//! menu placed everything 4 m ahead, on the floor, unturned. They are gone,
//! and this is the placement that is used.
//!
//! Boxes here are the renderer's: the unit box scaled by the transform, its
//! BOTTOM at `position.y` (`Mesh::box_xyz` is bottom-origin), rotated about
//! that point (the same convention as `uses::ray_hits_box`).

use super::shared::SharedPiece;
use super::site::{in_frame, PlanetSite};
use super::{Blueprint, BlueprintRegistry, Construction, Mount, Structure};
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
/// How far along the look ray the aim searches for the ground, metres. A
/// look that meets the ground farther out than this is treated as level.
pub const AIM_RAY_M: f32 = 60.0;
/// Two footprints must overlap by more than this (m) for one piece to rest
/// on the other. A few centimetres, so a wall that only touches a roof's
/// edge line, or a chest that clips a foundation's lip, does not lift the
/// piece onto it (a bed that overlapped a foundation by 1 cm used to float
/// 0.2 m over the floor). Walls overlap a roof they hold by half their
/// thickness, 7.5 cm for the thinnest, so they still count.
const OVERLAP_EPS_M: f32 = 0.05;
/// A piece rests only on pieces standing on the player's own storey: a
/// support counts when its BOTTOM is within this of the floor the player
/// aims at. More than a foundation's height or the fall of the ground across
/// one room, less than a storey (a wall is 3 m), so a roof finds the walls
/// of the room it covers and a wall placed on the ground floor never lands
/// on a foundation laid on the roof above.
pub const STOREY_STEP_M: f32 = 1.0;
/// How far apart the tops of two touching pieces of one category may be for
/// the new one to be levelled to the other (2026-09-28, [`level_top`]): enough
/// for the unevenness of the ground under a room, well under a storey.
pub const LEVEL_TOLERANCE_M: f32 = 0.3;
/// Two boxes closer than this at every face are the same footprint: a
/// second build there would be a double spend ([`occupied`]).
const SAME_BOX_M: f32 = 0.02;

/// A turn of `quarter_turns` times 90 degrees about the vertical.
pub fn quarter_turn(quarter_turns: u8) -> Quat {
    Quat::from_rotation_y(f32::from(quarter_turns % 4) * std::f32::consts::FRAC_PI_2)
}

/// The floor point the player is aiming at on a FLAT floor at `floor_y`
/// (the ship's decks): where the look ray meets the floor, kept between
/// [`MIN_REACH_M`] and [`MAX_REACH_M`] along the ground, or
/// [`DEFAULT_REACH_M`] straight ahead when the ray never meets the floor.
/// The same as [`aim_point_on_ground`] with a ground that is `floor_y`
/// everywhere.
pub fn aim_point(eye: Vec3, forward: Vec3, floor_y: f32) -> Vec3 {
    aim_point_on_ground(eye, forward, |_, _| floor_y)
}

/// The ground point the player is aiming at over a ground of any shape:
/// `ground(x, z)` is the ground's height at a point of the frame (on a
/// planet, the drawn terrain under it; aboard, the deck). The look ray is
/// followed to where it first meets the ground ([`ray_ground_hit`]); that
/// point's distance along the ground is kept between [`MIN_REACH_M`] and
/// [`MAX_REACH_M`]. A ray still going down when it runs out of search
/// ([`AIM_RAY_M`]) is aiming far, so it takes [`MAX_REACH_M`]; a ray looking
/// level or up takes [`DEFAULT_REACH_M`]. The returned point stands ON the
/// ground there: its y is `ground(x, z)`.
pub fn aim_point_on_ground(eye: Vec3, forward: Vec3, ground: impl Fn(f32, f32) -> f32) -> Vec3 {
    let flat = Vec3::new(forward.x, 0.0, forward.z);
    let flat = if flat.length_squared() > 1e-8 { flat.normalize() } else { Vec3::NEG_Z };
    let reach = match ray_ground_hit(eye, forward, &ground, AIM_RAY_M) {
        Some(hit) => Vec3::new(hit.x - eye.x, 0.0, hit.z - eye.z).length().clamp(MIN_REACH_M, MAX_REACH_M),
        None if forward.y < -1e-3 => MAX_REACH_M,
        None => DEFAULT_REACH_M,
    };
    let (x, z) = (eye.x + flat.x * reach, eye.z + flat.z * reach);
    Vec3::new(x, ground(x, z), z)
}

/// Where the ray `eye + t * dir` first goes below the ground `ground(x, z)`,
/// within `max_t` metres, or None. Marches in steps of half the ray's height
/// above the ground (never under 5 cm, never over 4 m), then halves the
/// last step twelve times, so the hit is good to well under a centimetre on
/// any ground not steeper than the ray.
pub fn ray_ground_hit(eye: Vec3, dir: Vec3, ground: &impl Fn(f32, f32) -> f32, max_t: f32) -> Option<Vec3> {
    let dir = dir.normalize_or_zero();
    if dir == Vec3::ZERO {
        return None;
    }
    let height = |t: f32| {
        let p = eye + dir * t;
        p.y - ground(p.x, p.z)
    };
    if height(0.0) <= 0.0 {
        return Some(eye);
    }
    let (mut t0, mut t) = (0.0_f32, 0.0_f32);
    while t <= max_t {
        let h = height(t);
        if h <= 0.0 {
            let (mut lo, mut hi) = (t0, t);
            for _ in 0..12 {
                let mid = 0.5 * (lo + hi);
                if height(mid) > 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return Some(eye + dir * hi);
        }
        t0 = t;
        t = (t + (h * 0.5).clamp(0.05, 4.0)).min(max_t + 1e-3);
    }
    None
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

/// Do two boxes overlap on the ground (x and z), by more than a few
/// centimetres ([`OVERLAP_EPS_M`])?
fn footprints_overlap(a: &(Vec3, Vec3), b: &(Vec3, Vec3)) -> bool {
    let ox = a.1.x.min(b.1.x) - a.0.x.max(b.0.x);
    let oz = a.1.z.min(b.1.z) - a.0.z.max(b.0.z);
    ox > OVERLAP_EPS_M && oz > OVERLAP_EPS_M
}

/// Which finished pieces a placement may rest on and level to, by whether
/// the server keeps them ([`placement_pose_where`]): every piece, what a
/// piece of the player's own uses.
pub fn any_piece(_kept: Option<&SharedPiece>) -> bool {
    true
}

/// Only the pieces the server keeps (`shared::SharedPiece`): what a piece
/// that will be kept by the server rests on and levels to
/// ([`placement_pose_where`]).
pub fn shared_pieces(kept: Option<&SharedPiece>) -> bool {
    kept.is_some()
}

/// The height `bp` rests at when its box is `candidate` (bottom at the
/// floor): `floor_y` for a floor piece; for `mount: OnTop`, the top of the
/// tallest FINISHED structure in `frame` whose blueprint category is in
/// `bp.snap_to`, whose footprint overlaps the candidate's, and which stands
/// on the player's storey (its bottom within [`STOREY_STEP_M`] of
/// `floor_y`); `floor_y` when none does. Scaffolds still going up hold
/// nothing.
pub fn rest_height(
    bp: &Blueprint,
    candidate: &Transform,
    floor_y: f32,
    world: &hecs::World,
    registry: &BlueprintRegistry,
    frame: Option<&PlanetSite>,
) -> f32 {
    rest_height_where(bp, candidate, floor_y, world, registry, frame, &any_piece)
}

/// [`rest_height`] on only the pieces `supports` accepts.
fn rest_height_where(
    bp: &Blueprint,
    candidate: &Transform,
    floor_y: f32,
    world: &hecs::World,
    registry: &BlueprintRegistry,
    frame: Option<&PlanetSite>,
    supports: &dyn Fn(Option<&SharedPiece>) -> bool,
) -> f32 {
    if bp.mount != Mount::OnTop || bp.snap_to.is_empty() {
        return floor_y;
    }
    let mine = world_aabb(candidate);
    let mut top: Option<f32> = None;
    for (_e, (s, tf, site, kept)) in world.query::<(&Structure, &Transform, Option<&PlanetSite>, Option<&SharedPiece>)>().iter() {
        if !in_frame(site, frame) || !supports(kept) {
            continue;
        }
        let Some(under) = registry.get(&s.blueprint_id) else { continue };
        if !bp.snap_to.iter().any(|c| *c == under.category) {
            continue;
        }
        let theirs = world_aabb(tf);
        if (theirs.0.y - floor_y).abs() > STOREY_STEP_M {
            continue;
        }
        if footprints_overlap(&mine, &theirs) {
            top = Some(top.map_or(theirs.1.y, |t| t.max(theirs.1.y)));
        }
    }
    top.unwrap_or(floor_y)
}

/// Where `bp` is built when the player aims at the floor point `at` with
/// `quarter_turns` turns, in `frame`: x and z on the metre grid, turned,
/// scaled to the blueprint's size, and resting at [`rest_height`]. It rests
/// on, and levels to, every finished piece; a piece that will be kept by the
/// server uses [`placement_pose_where`].
pub fn placement_pose(
    bp: &Blueprint,
    at: Vec3,
    quarter_turns: u8,
    world: &hecs::World,
    registry: &BlueprintRegistry,
    frame: Option<&PlanetSite>,
) -> Transform {
    placement_pose_where(bp, at, quarter_turns, world, registry, frame, any_piece)
}

/// [`placement_pose`] resting on, and levelling to, only the finished pieces
/// `supports` accepts, by whether the server keeps them (ship homes
/// increment 5, 2026-10-05).
///
/// WHY. Everyone near sees a piece the server keeps (`shared::SharedPiece`),
/// but of the pieces under it only the ones the server keeps too: a shared
/// wall resting on a private foundation (in this player's own home and
/// save) would hang 0.2 m over the deck for everyone else. And a shared wall
/// levelled to a private wall's top would leave a shared roof on it a sliver
/// of sky over the other shared walls, for everyone else. So the ghost of a
/// piece that will be shared is posed with [`shared_pieces`], and stands
/// right for everyone who sees it. A piece of the player's own uses
/// [`any_piece`] (`placement_pose`): it may rest on a shared foundation,
/// which everyone sees.
pub fn placement_pose_where(
    bp: &Blueprint,
    at: Vec3,
    quarter_turns: u8,
    world: &hecs::World,
    registry: &BlueprintRegistry,
    frame: Option<&PlanetSite>,
    supports: impl Fn(Option<&SharedPiece>) -> bool,
) -> Transform {
    let snapped = Vec3::new((at.x / GRID_M).round() * GRID_M, at.y, (at.z / GRID_M).round() * GRID_M);
    let mut tf = Transform { position: snapped, rotation: quarter_turn(quarter_turns), scale: Vec3::from_array(bp.size) };
    tf.position.y = rest_height_where(bp, &tf, at.y, world, registry, frame, &supports);
    if tf.position.y == at.y {
        level_top_where(bp, &mut tf, world, registry, frame, &supports);
    }
    tf
}

/// A piece standing on the GROUND (nothing under it holds it up) that touches
/// a finished piece of its own category and built height whose top is within
/// [`LEVEL_TOLERANCE_M`] of its own is made that much taller or shorter, its
/// bottom staying on the ground under it, so the two tops meet (2026-09-28).
/// On uneven ground each wall of a room otherwise stood at the ground under
/// it, and the roof, which rests on the tallest, left a sliver of sky over a
/// lower one. The first piece sets the level; a foundation still levels a
/// floor the old way. Several touching neighbours: the tallest top within
/// reach.
pub fn level_top(bp: &Blueprint, tf: &mut Transform, world: &hecs::World, registry: &BlueprintRegistry, frame: Option<&PlanetSite>) {
    level_top_where(bp, tf, world, registry, frame, &any_piece);
}

/// [`level_top`] to only the pieces `supports` accepts.
fn level_top_where(
    bp: &Blueprint,
    tf: &mut Transform,
    world: &hecs::World,
    registry: &BlueprintRegistry,
    frame: Option<&PlanetSite>,
    supports: &dyn Fn(Option<&SharedPiece>) -> bool,
) {
    let mine = world_aabb(tf);
    let my_top = mine.1.y;
    let mut target: Option<f32> = None;
    for (_e, (s, other, site, kept)) in world.query::<(&Structure, &Transform, Option<&PlanetSite>, Option<&SharedPiece>)>().iter() {
        if !in_frame(site, frame) || !supports(kept) {
            continue;
        }
        // Only a piece of the same kind and the same built height: walls with
        // walls (all 3 m), never a bed stretched to a chest's height.
        let same_kind = registry
            .get(&s.blueprint_id)
            .is_some_and(|b| b.category == bp.category && (b.size[1] - bp.size[1]).abs() < 1.0e-3);
        if !same_kind {
            continue;
        }
        let theirs = world_aabb(other);
        let d = theirs.1.y - my_top;
        if d.abs() > LEVEL_TOLERANCE_M || d.abs() < 1.0e-4 || !footprints_overlap(&mine, &theirs) {
            continue;
        }
        target = Some(target.map_or(theirs.1.y, |t: f32| t.max(theirs.1.y)));
    }
    if let Some(top) = target {
        tf.scale.y = (top - tf.position.y).max(0.1);
    }
}

/// Does a piece (finished, or a scaffold still going up) already stand in
/// `frame` with the same box as `pose`? Building there again would spend the
/// materials twice for one piece, so the ghost says so and the build is
/// refused.
pub fn occupied(world: &hecs::World, pose: &Transform, frame: Option<&PlanetSite>) -> bool {
    let mine = world_aabb(pose);
    let same = |tf: &Transform| {
        let theirs = world_aabb(tf);
        (theirs.0 - mine.0).abs().max_element() < SAME_BOX_M && (theirs.1 - mine.1).abs().max_element() < SAME_BOX_M
    };
    world
        .query::<(&Structure, &Transform, Option<&PlanetSite>)>()
        .iter()
        .any(|(_e, (_, tf, site))| in_frame(site, frame) && same(tf))
        || world
            .query::<(&Construction, &Transform, Option<&PlanetSite>)>()
            .iter()
            .any(|(_e, (_, tf, site))| in_frame(site, frame) && same(tf))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> BlueprintRegistry {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("blueprints").join("basic.ron");
        BlueprintRegistry::from_ron(&std::fs::read(path).unwrap()).unwrap()
    }

    /// Finish a piece in `world` at `at` with `turns`, the way the ghost
    /// places it and the ConstructionSystem builds it, in the home frame.
    fn build(world: &mut hecs::World, reg: &BlueprintRegistry, id: &str, at: Vec3, turns: u8) -> Transform {
        let bp = reg.get(id).unwrap_or_else(|| panic!("{id} in basic.ron"));
        let tf = placement_pose(bp, at, turns, world, reg, None);
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

    /// WALLS ON UNEVEN GROUND MEET AT THE TOP (2026-09-28). A wall on ground
    /// 5 cm lower than the one it touches is made 5 cm taller, its bottom on
    /// its own ground; one on higher ground shorter; a wall that does not
    /// touch, or whose top is further off than the tolerance (the storey
    /// above), keeps its size; and a roof over the room then rests at one
    /// height. Red check, run: skipping `level_top` leaves the low wall's top
    /// 5 cm short.
    #[test]
    fn walls_on_uneven_ground_meet_at_the_top() {
        let reg = shipped();
        let mut world = hecs::World::new();
        build(&mut world, &reg, "wood_wall", Vec3::new(0.0, 0.0, -2.0), 0);
        let low = build(&mut world, &reg, "wood_wall", Vec3::new(-2.0, -0.05, 0.0), 1);
        let (lo, hi) = world_aabb(&low);
        assert!((lo.y - -0.05).abs() < 1e-5 && (hi.y - 3.0).abs() < 1e-4, "bottom on its ground, top at the room's: {lo} {hi}");
        let high = build(&mut world, &reg, "wood_wall", Vec3::new(2.0, 0.08, 0.0), 1);
        let (lo, hi) = world_aabb(&high);
        assert!((lo.y - 0.08).abs() < 1e-5 && (hi.y - 3.0).abs() < 1e-4, "shorter on higher ground: {lo} {hi}");
        let apart = build(&mut world, &reg, "wood_wall", Vec3::new(20.0, -0.1, 20.0), 0);
        assert!((apart.scale.y - 3.0).abs() < 1e-5, "a wall touching nothing keeps its size");
        let far_off = build(&mut world, &reg, "wood_wall", Vec3::new(0.0, 1.0, 2.0), 0);
        assert!((far_off.scale.y - 3.0).abs() < 1e-5, "a top a metre off is not the same level");
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
        let bed = placement_pose(reg.get("bed").unwrap(), Vec3::ZERO, 0, &world, &reg, None);
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
        assert_eq!(placement_pose(reg.get("roof").unwrap(), Vec3::new(0.0, 0.5, 0.0), 0, &empty, &reg, None).position.y, 0.5);
    }

    /// OnTop rests only on the player's own storey (review of the shelter
    /// commit, BUG-102). A second storey stands on the first room's roof: a
    /// foundation laid on the roof at 3.2 m. A wall placed from the ground
    /// floor, whose `snap_to` is foundation, stays on the ground; placed from
    /// the upper floor it stands on the upper foundation. Red check, run:
    /// without the storey filter in `rest_height` the ground-floor wall
    /// jumps onto the upper foundation at 3.4 m and the first assertion
    /// fails.
    #[test]
    fn on_top_ignores_pieces_on_another_storey() {
        let reg = shipped();
        let mut world = hecs::World::new();
        room(&mut world, &reg, 0.0);
        build(&mut world, &reg, "roof", Vec3::ZERO, 0);
        // An upper foundation lying on the roof (its top at 3.2 m + 0.2 m).
        let f = reg.get("wood_foundation").unwrap();
        world.spawn((
            Transform { position: Vec3::new(0.0, 3.2, 0.0), rotation: Quat::IDENTITY, scale: Vec3::from_array(f.size) },
            Structure { blueprint_id: "wood_foundation".into(), health: 1.0, max_health: 1.0, provides: f.provides.clone(), uid: 0 },
        ));
        let wall = reg.get("wood_wall").unwrap();
        let ground_floor = placement_pose(wall, Vec3::new(0.0, 0.0, 1.0), 0, &world, &reg, None);
        assert_eq!(ground_floor.position.y, 0.0, "a ground-floor wall stays on the ground floor");
        let upstairs = placement_pose(wall, Vec3::new(0.0, 3.2, 1.0), 0, &world, &reg, None);
        assert!((upstairs.position.y - 3.4).abs() < 1e-4, "upstairs it stands on the upper foundation: {}", upstairs.position.y);
    }

    /// Overlaps of a few centimetres do not lift a piece: a chest whose box
    /// clips a foundation's edge by 3 cm stays on the floor, and one well
    /// onto the foundation rests on it. Red check, run: the old 1 cm
    /// threshold lifts the clipping chest to 0.2 m and the first assertion
    /// fails.
    #[test]
    fn a_hair_of_overlap_does_not_lift_a_piece() {
        let reg = shipped();
        let mut world = hecs::World::new();
        build(&mut world, &reg, "wood_foundation", Vec3::ZERO, 0); // spans x -2..2
        let chest = reg.get("storage_chest").unwrap(); // 1.0 x 0.6
        let clipping = Transform { position: Vec3::new(2.47, 0.0, 0.0), rotation: Quat::IDENTITY, scale: Vec3::from_array(chest.size) };
        assert_eq!(rest_height(chest, &clipping, 0.0, &world, &reg, None), 0.0, "3 cm over the edge: on the floor");
        let onto = Transform { position: Vec3::new(1.0, 0.0, 0.0), ..clipping };
        assert!((rest_height(chest, &onto, 0.0, &world, &reg, None) - 0.2).abs() < 1e-4);
    }

    /// A PIECE THAT WILL BE SHARED RESTS ONLY ON SHARED PIECES (ship homes
    /// increment 5, 2026-10-05). Everyone near sees a piece the server keeps,
    /// but only the pieces the server keeps: on a private foundation (in this
    /// player's own home and save) a shared wall would hang 0.2 m over the
    /// deck for all of them. On the deck: a private foundation at the origin
    /// with a private wall standing on it, and a shared foundation at x 10.
    /// `placement_pose_where` with `shared_pieces` stands a wall over the
    /// private foundation on the deck, and one over the shared foundation on
    /// it, 0.2 m up. It levels a wall only to shared walls: one on the deck
    /// touching the end of the private wall (top 3.2 m) keeps its own 3 m,
    /// where `placement_pose` (every piece, what a private piece uses) makes
    /// it 3.2 m to meet that top, and rests a wall on the private foundation.
    /// A shared roof on shared walls would otherwise show a sliver of sky
    /// over the other shared walls to everyone else.
    /// Seen red 2026-10-05 with the filter ignored: "a shared wall over a
    /// private foundation stands on the deck: left: 0.2, right: 0.0".
    #[test]
    fn a_shared_wall_rests_on_a_shared_foundation_not_a_private_one() {
        let reg = shipped();
        let mut world = hecs::World::new();
        build(&mut world, &reg, "wood_foundation", Vec3::ZERO, 0); // private: x -2..2, z -2..2
        let kept = build(&mut world, &reg, "wood_foundation", Vec3::new(10.0, 0.0, 0.0), 0);
        let marked: Vec<hecs::Entity> =
            world.query::<(&Structure, &Transform)>().iter().filter(|(_, (_, t))| t.position == kept.position).map(|(e, _)| e).collect();
        world.insert_one(marked[0], SharedPiece { piece_id: 1, frame: "plot:p1".into(), mine: true }).unwrap();
        let private_wall = build(&mut world, &reg, "wood_wall", Vec3::new(0.0, 0.0, -2.0), 0);
        assert!((world_aabb(&private_wall).1.y - 3.2).abs() < 1e-4, "the private wall stands on the private foundation");
        let wall = reg.get("wood_wall").unwrap();
        let shared_only = shared_pieces;

        let over_private = placement_pose_where(wall, Vec3::new(0.0, 0.0, 1.0), 0, &world, &reg, None, shared_only);
        assert_eq!(over_private.position.y, 0.0, "a shared wall over a private foundation stands on the deck");
        let over_shared = placement_pose_where(wall, Vec3::new(10.0, 0.0, 1.0), 0, &world, &reg, None, shared_only);
        assert!((over_shared.position.y - 0.2).abs() < 1e-4, "on the shared foundation: {}", over_shared.position.y);

        // Touching the private wall's end (x 1.9..2.1, z -6..-2), clear of the private foundation.
        let touching = placement_pose_where(wall, Vec3::new(2.0, 0.0, -4.0), 1, &world, &reg, None, shared_only);
        assert_eq!((touching.position.y, touching.scale.y), (0.0, 3.0), "levelled only to shared walls");
        let private = placement_pose(wall, Vec3::new(2.0, 0.0, -4.0), 1, &world, &reg, None);
        assert!((private.scale.y - 3.2).abs() < 1e-4, "a private wall meets the private top: {}", private.scale.y);
        let private_over = placement_pose(wall, Vec3::new(0.0, 0.0, 1.0), 0, &world, &reg, None);
        assert!((private_over.position.y - 0.2).abs() < 1e-4, "a private wall rests on the private foundation");
    }

    /// A second build of the same box in the same frame is a double spend:
    /// `occupied` sees a finished piece and a scaffold alike, and not a
    /// piece of another size, another spot, or another frame.
    #[test]
    fn the_same_box_twice_is_occupied() {
        let reg = shipped();
        let mut world = hecs::World::new();
        let wall = build(&mut world, &reg, "wood_wall", Vec3::new(0.0, 0.0, 2.0), 0);
        assert!(occupied(&world, &wall, None));
        let turned = placement_pose(reg.get("wood_wall").unwrap(), Vec3::new(0.0, 0.0, 2.0), 1, &world, &reg, None);
        assert!(!occupied(&world, &turned, None), "a turned wall at the same spot is a different box");
        let site = PlanetSite { body: "earth".into(), origin: glam::DVec3::new(0.0, 6.371e6, 0.0) };
        assert!(!occupied(&world, &wall, Some(&site)), "a planet site is another frame");
        let mut scaffolds = hecs::World::new();
        scaffolds.spawn((wall.clone(), Construction { blueprint_id: "wood_wall".into(), progress: 0.0, build_time: 4.0, builder_key: None }));
        assert!(occupied(&scaffolds, &wall, None), "a scaffold going up counts");
    }

    /// The aim point on a flat floor: where the look ray meets the floor,
    /// clamped along the ground; straight ahead at 4 m when looking level or
    /// up.
    #[test]
    fn the_aim_point_follows_the_look_ray_to_the_floor() {
        let eye = Vec3::new(0.0, 1.7, 0.0);
        let down = Vec3::new(0.0, -1.7, -3.0).normalize();
        let p = aim_point(eye, down, 0.0);
        assert!((p - Vec3::new(0.0, 0.0, -3.0)).length() < 1e-3, "{p}");
        let level = aim_point(eye, Vec3::X, 0.0);
        assert!((level - Vec3::new(DEFAULT_REACH_M, 0.0, 0.0)).length() < 1e-4, "{level}");
        let feet_down = aim_point(eye, Vec3::new(0.0, -1.0, -0.01).normalize(), 0.0);
        assert!((feet_down.z + MIN_REACH_M).abs() < 1e-3, "never inside the player: {feet_down}");
        let far = aim_point(eye, Vec3::new(0.0, -0.01, -1.0).normalize(), 0.0);
        assert!((far.z + MAX_REACH_M).abs() < 1e-3, "{far}");
    }

    /// On a planet the aim meets the GROUND under the crosshair, not a flat
    /// plane at the feet (BUG-102 review). On a slope rising 0.25 m per
    /// metre ahead, looking down 30 degrees from a 1.7 m eye, the ray meets
    /// the rising ground well short of where it would meet the plane at the
    /// feet, and the aimed point stands ON the slope. Red check, run: aiming
    /// with `aim_point(eye, fwd, feet_y)` (the flat plane at the feet, what
    /// the ship's decks use) lands 2.9 m ahead at y 0 instead, and both
    /// assertions fail.
    #[test]
    fn on_a_planet_the_aim_meets_the_ground_under_the_crosshair() {
        let slope = |_x: f32, z: f32| -0.25 * z; // rises toward -Z (ahead)
        let eye = Vec3::new(0.0, 1.7, 0.0);
        let fwd = Vec3::new(0.0, -(30.0_f32.to_radians()).sin(), -(30.0_f32.to_radians()).cos());
        let p = aim_point_on_ground(eye, fwd, slope);
        // Analytic hit: 1.7 - t sin30 = 0.25 t cos30 -> t = 1.7 / (0.5 + 0.2165).
        let t = 1.7 / (0.5 + 0.25 * 30.0_f32.to_radians().cos());
        let want_z = -t * 30.0_f32.to_radians().cos();
        assert!((p.z - want_z).abs() < 0.01, "hit at z {} want {want_z}", p.z);
        assert!((p.y - slope(p.x, p.z)).abs() < 1e-6, "the point stands on the ground: {p}");
        // A dip: the ground falls away 3 m past a ledge 2 m ahead; aiming
        // past the ledge finds the lower ground, not a plane at the feet.
        let ledge = |_x: f32, z: f32| if z < -2.0 { -3.0 } else { 0.0 };
        let q = aim_point_on_ground(eye, Vec3::new(0.0, -0.5, -1.0).normalize(), ledge);
        assert!((q.y + 3.0).abs() < 1e-6 && q.z < -2.0, "aimed into the dip: {q}");
    }
}

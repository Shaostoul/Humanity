//! THE DEV CROWD WALKING SOMEWHERE (crowd navigation rung 1, 2026-11-13; docs/design/crowd-navigation.md,
//! section 10.1): the `crowd` verb's walking stand-ins on the navigation mesh, through doors, round
//! furniture and each other, instead of on circles through the tables (the journal, 2026-11-11).
//!
//! Each frame with walkers (`step`, from `animation::draw_people`), it gathers what the mesh is
//! built from on the walkers' storey: the walls the walk collides with (`EngineState::wall_colliders`),
//! the doors that do not open for a walker (locked, manual, fixed), the furniture (every placed
//! machine's turned box, kept by the machine rebuild counter `HomeDraw::edits` and read again every
//! frame with reuse off), and the ship's floors from its layout. `NavCache` keeps the mesh while
//! the fingerprint of those holds (and builds it every frame with the Dev page's "Turn off all
//! reuse" switch off), then the walkers step (`humanity_ship::crowd_nav::crowd::step`). The cost is
//! the stage `cpu.crowd_nav`, inside `cpu.animation` (the Performance page's "Characters" row).
//!
//! Permanent dev tooling (CLAUDE.md: never trimmed): the rigs measure crowds with it, and it is the
//! first user of the crate's navigation, which the relay's crew adopt next (the design, section 9).
//!
//! The furniture list is also the player's (2026-11-13, the operator: "We want the ship to feel
//! alive"): `refresh_furniture` keeps it current with or without a crowd, and the walk's collision
//! (lib.rs, beside the shut doors and the built pieces) stops the body at every piece across its
//! knee-to-eye band through `furniture_walls_near`, one list and one rule for walkers and player.

use crate::engine::animation::{crowd_place, CrowdFigure, CrowdMotion};
use crate::engine::state::EngineState;
use glam::{Vec2, Vec3};
use humanity_ship::crowd_nav::{self, crowd::step as crowd_step, Agent, CrowdParams, Footprint, NavCache, NavInput};
use humanity_ship::wall_collision::WallSegment;

/// A walker of the dev crowd: its place on the mesh and the height of the floor it walks on.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NavWalker {
    pub agent: Agent,
    pub feet_y: f32,
}

impl NavWalker {
    /// Where its feet are, world (the ship's frame, the one the camera walks in).
    pub(crate) fn feet(&self) -> Vec3 {
        Vec3::new(self.agent.pos.x, self.feet_y, self.agent.pos.y)
    }
}

/// The crowd's navigation state, beside the crowd (`AnimationRuntime::walk`).
#[derive(Debug, Default)]
pub(crate) struct CrowdWalk {
    pub(crate) nav: NavCache,
    /// The furniture's footprints and the machine rebuild count they were read at.
    furniture: Vec<Footprint>,
    furniture_stamp: Option<u64>,
    /// How the walkers walk (data/crowd/walking.ron, read each time a crowd is made).
    pub(crate) params: CrowdParams,
}

impl CrowdWalk {
    /// The furniture's footprints as last made current (`refresh_furniture`).
    pub(crate) fn furniture(&self) -> &[Footprint] {
        &self.furniture
    }
}

/// The walking rules from data/crowd/walking.ron (`CrowdParams`'s numbers, with their sources in
/// the design, sections 5 and 7); the defaults, said in the log, when it cannot be read.
pub(crate) fn load_params() -> CrowdParams {
    let path = humanity_core::storage::data_dir().join("crowd/walking.ron");
    match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| ron::from_str::<CrowdParams>(&t).map_err(|e| e.to_string())) {
        Ok(p) => p,
        Err(e) => {
            log::warn!("Crowd: {} did not load ({e}); walking by the built-in defaults", path.display());
            CrowdParams::default()
        }
    }
}

/// True where the crowd can walk on a navigation mesh: aboard, where the walls are.
pub(crate) fn can_walk_here(state: &EngineState) -> bool {
    state.aboard_station && !state.wall_colliders.is_empty()
}

/// Make the crowd's walking stand-ins walkers (the `crowd` verb, aboard): each starts where its
/// circle would have put it now, roams `roam` metres round `origin` at its circle's speed, and
/// stands a moment at each destination. A stand-in standing still (speed 0, no turn) becomes a
/// walker that stands, and steps aside for the others; one turning on the spot keeps turning.
pub(crate) fn make_walkers(figures: &mut [CrowdFigure], origin: Vec3, roam: f32, now: f32) {
    let home = Vec2::new(origin.x, origin.z);
    for (i, f) in figures.iter_mut().enumerate() {
        let speed = match &f.motion {
            CrowdMotion::Circle { speed, .. } => speed.abs(),
            CrowdMotion::Turn { rate, .. } if *rate == 0.0 => 0.0,
            _ => continue,
        };
        let (feet, facing) = crowd_place(f, now);
        let mut agent = Agent::new(Vec2::new(feet.x, feet.z), home, roam, speed, i as u64 + 1);
        agent.facing = facing;
        f.motion = CrowdMotion::Nav(Box::new(NavWalker { agent, feet_y: origin.y }));
    }
}

/// The walkers as door actors (their feet and how fast they cross the floor), so an automatic
/// door opens for them as they come to it, as it does for another player.
pub(crate) fn door_actors(state: &EngineState) -> impl Iterator<Item = (Vec3, f32)> + '_ {
    state.animation.crowd.iter().filter_map(|f| match &f.motion {
        CrowdMotion::Nav(w) => Some((w.feet(), w.agent.vel.length())),
        _ => None,
    })
}

/// Every placed machine and piece of furniture as a turned box on the floor, the way it is drawn
/// (`Footprint::of_placed`).
fn furniture_of(state: &EngineState) -> Vec<Footprint> {
    let Some(placed) = crate::engine::home_meshes::placements_now(state) else {
        return Vec::new();
    };
    placed.iter().map(Footprint::of_placed).collect()
}

/// The furniture's footprints made current: read again when the machines were rebuilt
/// (`HomeDraw::edits`), and every frame with the Dev page's "Turn off all reuse" switch off. The
/// one list the crowd's mesh and the player's walk read (2026-11-13: the player stops at furniture
/// the way the crowd does, docs/design/crowd-navigation.md, "His answers").
pub(crate) fn refresh_furniture(state: &mut EngineState) {
    let stamp = state.home_draw.edits;
    if !humanity_core::reuse::enabled() || state.animation.walk.furniture_stamp != Some(stamp) {
        let furniture = furniture_of(state);
        let walk = &mut state.animation.walk;
        walk.furniture = furniture;
        walk.furniture_stamp = Some(stamp);
    }
}

/// How far round the player's move the furniture is gathered for the walk, metres: the longest
/// walking move of a frame and the body's radius, with room to spare (a placement, longer than
/// `wall_collision::PLACEMENT_M`, is settled where it lands, which this covers too).
const PLAYER_FURNITURE_REACH_M: f32 = 8.0;

/// The furniture as the player's walk blockers (`Footprint::walls`) near where the body is
/// (`at`, its eye): none while an interaction holds the body (seated on a chair, lying on a bed,
/// getting up), since the seat it sits on stands where the body is; the walk takes the body back
/// at the approach point, outside the piece.
pub(crate) fn furniture_walls_near(furniture: &[Footprint], at: Vec3, engaged: bool) -> Vec<WallSegment> {
    if engaged {
        return Vec::new();
    }
    let feet_y = at.y - humanity_ship::wall_collision::EYE_HEIGHT_M;
    let r = PLAYER_FURNITURE_REACH_M;
    furniture
        .iter()
        .filter(|f| f.stops_body(feet_y))
        .filter(|f| {
            let (x0, z0, x1, z1) = f.bounds();
            x1 >= at.x - r && x0 <= at.x + r && z1 >= at.z - r && z0 <= at.z + r
        })
        .flat_map(|f| f.walls())
        .collect()
}

/// The doors that do not open for a walker, as walls: a locked one, a manual one, one whose style
/// does not open. An automatic, unlocked door is a gap in the mesh; it opens as a walker comes.
fn doors_shut_to_walkers(state: &EngineState) -> impl Iterator<Item = WallSegment> + '_ {
    state.door_panels.iter().enumerate().filter_map(|(i, (p, _))| {
        if p.is_window {
            return None;
        }
        let locked = crate::engine::home_meshes::door_locked_now(p, state.door_locks.get(i));
        let opens = humanity_systems::door_anim::is_operable(&p.style) && p.auto_open && !locked;
        (!opens).then(|| humanity_ship::wall_collision::door_panel_segment(p.center, p.rotation, p.size))
    })
}

/// One frame of the crowd's walk: the mesh made sure of, every walker stepped `dt` seconds.
pub(crate) fn step(state: &mut EngineState, dt: f32) {
    // The walkers' region and storey: round every walker's roam, never where they stand, so the
    // mesh holds while they walk (`crowd_nav::crowd::region`).
    let walkers = || {
        state.animation.crowd.iter().filter_map(|f| match &f.motion {
            CrowdMotion::Nav(w) => Some(w.as_ref()),
            _ => None,
        })
    };
    let feet_y = walkers().next().map(|w| w.feet_y);
    let region = humanity_ship::crowd_nav::crowd::region(walkers().map(|w| &w.agent));
    let (Some(feet_y), Some((lo, hi))) = (feet_y, region) else {
        // No walkers: the mesh they walked on is let go.
        if state.animation.walk.nav.mesh().is_some() {
            state.animation.walk.nav.clear();
        }
        return;
    };
    let _cost = humanity_core::frame_costs::stage("cpu.crowd_nav");
    let reuse = humanity_core::reuse::enabled();
    refresh_furniture(state);
    // The walls near the region (the mesh reads only those that reach it and stop a body on this
    // storey), and the doors shut to walkers.
    let (x0, z0, x1, z1) = (lo.x - 2.0, lo.y - 2.0, hi.x + 2.0, hi.y + 2.0);
    let mut walls: Vec<WallSegment> = state
        .wall_colliders
        .iter()
        .filter(|w| w.a.0.max(w.b.0) >= x0 && w.a.0.min(w.b.0) <= x1 && w.a.1.max(w.b.1) >= z0 && w.a.1.min(w.b.1) <= z1)
        .copied()
        .collect();
    walls.extend(doors_shut_to_walkers(state));
    let floor = state.gui_state.ship_structure.as_ref().map(|s| crowd_nav::floor::floor_rects(s, feet_y)).unwrap_or_default();
    let walk = &mut state.animation.walk;
    let input = NavInput { min: (lo.x, lo.y), max: (hi.x, hi.y), feet_y, walls: &walls, footprints: &walk.furniture, floor: &floor, radius: crowd_nav::BODY_RADIUS_M };
    if walk.nav.ensure(&input, reuse) {
        if let Some(m) = walk.nav.mesh() {
            log::info!(
                "Crowd: navigation mesh built ({:.1} ms; {} x {} cells of {:.2} m, {} walkable, {} rectangles; {} walls, {} pieces of furniture, {} floors)",
                walk.nav.last_build_ms,
                m.w,
                m.h,
                m.cell,
                m.walkable_cells(),
                m.polys.len(),
                input.walls_here().count(),
                input.footprints_here().count(),
                floor.len()
            );
        }
    }
    let Some(mesh) = walk.nav.mesh() else { return };
    // The walkers out of the crowd, stepped together, and back.
    let mut slots = Vec::new();
    let mut agents = Vec::new();
    for (i, f) in state.animation.crowd.iter_mut().enumerate() {
        if let CrowdMotion::Nav(w) = &mut f.motion {
            slots.push(i);
            agents.push(std::mem::take(&mut w.agent));
        }
    }
    crowd_step(mesh, &mut agents, dt, &walk.params);
    for (i, a) in slots.into_iter().zip(agents) {
        if let CrowdMotion::Nav(w) = &mut state.animation.crowd[i].motion {
            w.agent = a;
        }
    }
}

#[cfg(test)]
mod tests {
    //! The player stops at furniture (2026-11-13; docs/design/crowd-navigation.md, "His answers"):
    //! the walk's collision fed the crowd's own footprints, on the shipped ship and its machines.
    use super::*;
    use humanity_animation::interact::{AnchorKind, Anchors};
    use humanity_ship::machines::{MachineHome, PlacedMachine};
    use humanity_ship::wall_collision::{resolve, EYE_HEIGHT_M, PLAYER_RADIUS};
    use std::path::{Path, PathBuf};

    fn data() -> PathBuf {
        Path::new(env!("HUMANITY_REPO_ROOT")).join("data")
    }

    /// The shipped ship's walls and every machine placed on it, as the game places them.
    fn shipped() -> (Vec<WallSegment>, Vec<PlacedMachine>, MachineHome) {
        let ship = humanity_ship::ship_structure::ShipStructure::load_and_assemble_shipped(&data(), None).expect("the shipped ship assembles");
        let home = humanity_systems::grow_machines::load_home(&data().join("machines/home.ron")).expect("the shipped machines load");
        let placed = home.placements(&std::collections::HashMap::new(), Some(&ship.zone_rects()));
        (humanity_ship::wall_collision::ship_wall_segments(&ship), placed, home)
    }

    fn placed<'a>(all: &'a [PlacedMachine], id: &str) -> &'a PlacedMachine {
        all.iter().find(|p| p.id == id).unwrap_or_else(|| panic!("the shipped ship places {id}"))
    }

    /// The player's eye walked from `from` toward `to` at a walking 1.4 m/s for up to `secs`
    /// seconds at 60 frames a second, each frame's move held by the walk's collision the way the
    /// frame holds it (lib.rs: the move, then `resolve` with the walls and, beside the doors, the
    /// furniture near the body).
    fn walk(from: Vec3, to: Vec3, walls: &[WallSegment], furniture: &[Footprint], secs: f32) -> Vec3 {
        let dt = 1.0 / 60.0;
        let mut p = from;
        for _ in 0..(secs / dt) as usize {
            let d = Vec3::new(to.x - p.x, 0.0, to.z - p.z);
            let len = d.length();
            if len < 1e-4 {
                break;
            }
            let next = p + d / len * (1.4 * dt).min(len);
            let blockers = furniture_walls_near(furniture, next, false);
            p = resolve(p, next, PLAYER_RADIUS, walls, &blockers);
        }
        p
    }

    /// How near a seat's or a bed's approach the body must come, metres: within a body's radius and a
    /// little, well inside every anchor's `reach_m`.
    const NEAR_M: f32 = 0.35;

    /// The eye of a body standing on the floor the piece stands on (its foot: an upstairs bed's
    /// storey, not its zone's ground floor, which `floor_y` is).
    fn eye_over(p: &PlacedMachine) -> f32 {
        p.pos.1 + EYE_HEIGHT_M
    }

    /// RED, 2026-11-13, before the walk read the furniture (`furniture_walls_near` returning none):
    /// "walking straight at the table from Vec3(40.5, 1.7, 34.0) it stood 0.800 m inside its edge
    /// (-0.500 m from it, the body's radius 0.3)".
    #[test]
    fn the_player_cannot_walk_into_a_tables_footprint() {
        let (_, all, _) = shipped();
        let at = placed(&all, "dining_table_3");
        let table = Footprint::of_placed(at);
        let y = eye_over(at);
        // The shipped table and the same table turned 37 degrees, walked at from each side and
        // across each corner, 2.5 m out, toward its middle for 5 s (twice the walk it needs).
        for fp in [table, Footprint { yaw: table.yaw + 37f32.to_radians(), ..table }] {
            let mid = Vec3::new(fp.centre.0, y, fp.centre.1);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::FRAC_PI_4;
                let from = mid + Vec3::new(a.cos(), 0.0, a.sin()) * 2.5;
                let end = walk(from, mid, &[], &[fp], 5.0);
                let d = fp.distance(end.x, end.z);
                assert!(
                    d >= PLAYER_RADIUS - 0.02,
                    "walking straight at the table from {from:?} it stood {:.3} m inside its edge ({d:.3} m from it, the body's radius {PLAYER_RADIUS})",
                    (PLAYER_RADIUS - d).max(0.0)
                );
            }
        }
    }

    /// EVERY SHIPPED SEAT AND BED STILL REACHED. Each piece's own footprint leaves its approach (where
    /// a person stands before sitting or lying, data/characters/interactions.ron) standable: the body
    /// stands there within `NEAR_M` with the ship's walls and that piece alone. Among all the
    /// furniture the body stands within the anchor's `reach_m` of it, which is all E needs to grant
    /// it (engine/interact.rs `press_e`): a desk a chair is tucked under may crowd the approach
    /// (chair_5's stands 0.245 m off, chair_22's 0.964 m: their approaches were authored under the
    /// desk), never beyond reach. And seated or lying, the hold reads no furniture.
    /// RED, 2026-11-13, with every footprint taken at twice its size in `Footprint::walls`: "the
    /// bed_lie approach of bed_1 is held 1.139 m short of it by its own footprint".
    #[test]
    fn the_player_still_reaches_every_seat_and_bed_on_the_ship() {
        let (walls, all, home) = shipped();
        let anchors = Anchors::parse(&std::fs::read_to_string(data().join("characters/interactions.ron")).unwrap()).expect("interactions.ron parses");
        let types = home.instance_types();
        let furniture: Vec<Footprint> = all.iter().map(Footprint::of_placed).collect();
        let (mut seats, mut beds) = (0, 0);
        for p in &all {
            let Some(def) = types.get(&p.id).and_then(|t| home.catalog.get(t)) else { continue };
            let own = [Footprint::of_placed(p)];
            for id in &def.anchors {
                let Some(a) = anchors.index(id).map(|i| &anchors.anchors[i]) else { continue };
                if !matches!(a.kind, AnchorKind::Sit | AnchorKind::Lie) {
                    continue;
                }
                let turn = glam::Quat::from_rotation_y(p.rotation.to_radians());
                let point = |l: [f32; 3]| Vec3::from(p.pos) + turn * Vec3::from(l);
                let approach = point(a.approach);
                let eye = Vec3::new(approach.x, eye_over(p), approach.z);
                let off_by = |pieces: &[Footprint]| {
                    let stood = resolve(eye, eye, PLAYER_RADIUS, &walls, &furniture_walls_near(pieces, eye, false));
                    Vec3::new(stood.x - eye.x, 0.0, stood.z - eye.z).length()
                };
                let own_off = off_by(&own);
                assert!(own_off <= NEAR_M, "the {} approach of {} is held {own_off:.3} m short of it by its own footprint", a.id, p.id);
                let off = off_by(&furniture);
                assert!(off <= a.reach_m, "among the furniture the body stands {off:.3} m from the {} approach of {}, beyond E's reach of {} m", a.id, p.id, a.reach_m);
                match a.kind {
                    AnchorKind::Lie => beds += 1,
                    _ => seats += 1,
                }
                // Seated or lying, the body is where the piece stands: the hold is never pushed off it.
                let root = point(a.root);
                let held = Vec3::new(root.x, eye_over(p), root.z);
                assert!(furniture_walls_near(&furniture, held, true).is_empty(), "an interaction's hold reads no furniture");
            }
        }
        assert!(seats >= 4 && beds >= 1, "the shipped ship has chairs, a couch and a bed to reach ({seats} seats, {beds} beds)");
    }

    /// RED, 2026-11-13, with the footprint's walls standing at every height
    /// (`WallSegment::at_every_height` in `Footprint::walls`, and `furniture_walls_near` not leaving out
    /// what stands under the knee): "walking across the rug it stopped 4.30 m short of the far side".
    #[test]
    fn a_rug_does_not_block_the_walk() {
        let (_, all, _) = shipped();
        let p = placed(&all, "rug_1");
        let rug = Footprint::of_placed(p);
        let y = eye_over(p);
        let (x0, z0, x1, z1) = rug.bounds();
        let from = Vec3::new(x0 - 1.0, y, (z0 + z1) * 0.5);
        let to = Vec3::new(x1 + 1.0, y, (z0 + z1) * 0.5);
        let end = walk(from, to, &[], &[rug], 10.0);
        let short = Vec3::new(to.x - end.x, 0.0, to.z - end.z).length();
        assert!(short < 0.01, "walking across the rug it stopped {short:.2} m short of the far side");
    }
}

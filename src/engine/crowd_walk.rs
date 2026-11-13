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
/// (`home_meshes::machine_mesh`): a box its size, a cylinder or a sphere its radius (`size.0`)
/// each way, a sphere standing a radius under its centre.
fn furniture_of(state: &EngineState) -> Vec<Footprint> {
    let Some(placed) = crate::engine::home_meshes::placements_now(state) else {
        return Vec::new();
    };
    placed
        .iter()
        .map(|p| {
            let (sx, sy, sz) = p.size;
            let (half, foot, top) = match p.shape.as_str() {
                "sphere" => ((sx, sx), p.pos.1 - sx, p.pos.1 + sx),
                "cylinder" => ((sx, sx), p.pos.1, p.pos.1 + sy),
                "pyramid" => ((sx * 0.5, sx * 0.5), p.pos.1, p.pos.1 + sy),
                _ => ((sx * 0.5, sz * 0.5), p.pos.1, p.pos.1 + sy),
            };
            Footprint { centre: (p.pos.0, p.pos.2), half, yaw: p.rotation.to_radians(), y0: foot, y1: top }
        })
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
    let stamp = state.home_draw.edits;
    if !reuse || state.animation.walk.furniture_stamp != Some(stamp) {
        let furniture = furniture_of(state);
        let walk = &mut state.animation.walk;
        walk.furniture = furniture;
        walk.furniture_stamp = Some(stamp);
    }
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

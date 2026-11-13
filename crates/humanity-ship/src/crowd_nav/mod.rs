//! CROWD NAVIGATION (rung 1, 2026-11-13; the whole plan is docs/design/crowd-navigation.md):
//! people who walk somewhere, through doors, round furniture and each other.
//!
//! Three layers, each the published technique a 2030 game uses, scoped to one storey of the ship:
//!
//! 1. THE NAVIGATION MESH (`mesh`): the walkable floor of a region, built from what the walk
//!    already collides with (the walls and doors as `wall_collision::WallSegment`s, the furniture's
//!    turned footprints, the floors of the zones and corridors), rasterised the way Recast does it
//!    (Mononen, Recast Navigation, 2009) into cells whose centres keep a body's radius clear of
//!    every obstacle, then merged into convex rectangles joined by portals. Rebuilt only when what
//!    it was built from changes (`NavCache`, keyed by a fingerprint of its inputs), and every time
//!    while the Dev page's "Turn off all reuse" switch is off (the caller passes
//!    `humanity_core::reuse::enabled()`).
//! 2. THE PATH (`path`): A* over the rectangles (Hart, Nilsson and Raphael, 1968), the corridor of
//!    rectangles string-pulled into the shortest line through their portals by the funnel algorithm
//!    (Lee and Preparata, 1984; the "simple stupid funnel" form, Mononen, 2010).
//! 3. AVOIDANCE (`avoid`): optimal reciprocal collision avoidance between people (ORCA; van den Berg,
//!    Guy, Lin and Manocha, "Reciprocal n-Body Collision Avoidance", ISRR 2009, published 2011): each
//!    person takes half the responsibility for missing each neighbour, and keeps clear of the
//!    navigation mesh's edge, solved as a small linear program every step.
//!
//! `crowd` runs them together: each person has a destination, a path to it, a preferred walking
//! speed, and the velocity avoidance chooses; their position is kept on the walkable cells, so no
//! body centre ever enters furniture or a wall. Plain geometry with no GPU in it, in the crate the
//! relay compiles too, so the crew the relay walks can adopt it (the design, section 9).

pub mod avoid;
pub mod crowd;
pub mod floor;
pub mod mesh;
pub mod path;

#[cfg(test)]
mod tests;

pub use crowd::{Agent, CrowdParams};
pub use mesh::{FloorRect, Footprint, NavCache, NavInput, NavMesh};

/// A walking person's radius, metres: the clearance the navigation mesh keeps from every obstacle
/// and the disc avoidance keeps people apart by. Fruin's body ellipse (Pedestrian Planning and
/// Design, 1971) is 0.61 m across the shoulders by 0.46 m deep, an area of 0.22 m2; a disc of the
/// same area is 0.265 m in radius. 0.25 lets a body brush past a table edge as a person turns a
/// shoulder, the radius most crowd models use (the design, section 5).
pub const BODY_RADIUS_M: f32 = 0.25;

/// The navigation mesh's cell, metres: fine enough that a 0.8 m interior door, less a body's
/// radius at each side, still leaves cells to walk through.
pub const CELL_M: f32 = 0.1;

/// The most cells a navigation mesh holds: a region larger than this many cells at `CELL_M` is
/// built with larger cells instead (`NavInput::cell_for`), so a crowd told to roam a whole deck
/// cannot ask for a grid that takes seconds to build.
pub const MAX_CELLS: usize = 1 << 20;

//! Pulleys: the block and tackle as real gear, and the configurator that designs one for any
//! load and any puller (docs/design/pulleys.md; rung 1, 2026-11-13).
//!
//! The operator, 2026-11-12: "we want a pulley that makes me life something 1,000 times heavier
//! and it auto calculates the dimensions of everything", drawn "exclusively with math, no .obj
//! like file", and "recommend the best configuration for the scale. Like a human has to use this
//! thing. Or a truck can use it. Or spaceship."
//!
//! - `data`: the rows (pullers, ropes, bearings, block rules, geared hoists) from
//!   `data/pulleys/pulleys.ron`, each number with its source beside it there.
//! - `tackle`: the physics (real advantage with friction compounding per sheave, rope taken in,
//!   the gear's own weight, working load limits) and the configurator (`recommend`).
//! - `geometry`: every block dimension from the rope and the load; the mesh and the collision
//!   shapes from those numbers; a store that builds a shape once per configuration and obeys the
//!   reuse switch.
//!
//! The crate speaks in word keys and numbers; the Calculator page's Pulleys panel turns them into
//! words (`src/gui/pages/calculator/pulleys.rs`).

pub mod data;
pub mod geometry;
pub mod tackle;

#[cfg(test)]
mod tests;

pub use data::{Bearing, BlockRules, Hoist, Material, Puller, PulleyData, Rope, World};
pub use geometry::{Aabb, BlockDims, Capsule, MeshData, ShapeStore, StageShape};
pub use tackle::{evaluate, recommend, stage_real_ma, Advice, Inputs, Mode, Outcome, Refusal, StageDesign, Verdict};

/// Build every stage's shape for an outcome (the fixed block at each stage's origin).
pub fn stage_shapes(out: &Outcome, lead_m: f64) -> Vec<StageShape> {
    out.stages.iter().map(|s| StageShape::build(s.parts, &s.upper, &s.lower, s.span_m, lead_m)).collect()
}

/// The numbers a set of stage shapes is built from: its key in a `ShapeStore`.
pub fn shape_key(out: &Outcome, lead_m: f64) -> u64 {
    let mut numbers = vec![lead_m];
    for s in &out.stages {
        numbers.extend_from_slice(&[s.parts as f64, s.span_m, s.rope_d_mm, s.upper.wll_n, s.lower.wll_n, s.upper.tread_d]);
    }
    geometry::shape_key(&numbers)
}

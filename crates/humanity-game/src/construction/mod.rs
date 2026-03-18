//! Parametric construction system — CSG operations, blueprints, structural analysis.
//!
//! Building materials loaded from `data/materials.csv`.

pub mod csg;
pub mod blueprint;
pub mod structural;
pub mod routing;

/// Top-level construction system.
pub struct ConstructionSystem {
    // TODO: active build state, placed structures
}

impl ConstructionSystem {
    pub fn new() -> Self {
        Self {}
    }
}

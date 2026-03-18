//! AI system — behavior trees, flow fields, off-screen autonomy.
//!
//! Behavior tree definitions loaded from `data/behaviors.ron`.

pub mod behavior;
pub mod flow_field;
pub mod autonomy;

/// AI system coordinator.
pub struct AiSystem {
    // TODO: active behavior trees, flow field cache
}

impl AiSystem {
    pub fn new() -> Self {
        Self {}
    }
}

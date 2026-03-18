//! Off-screen NPC simulation — NPCs continue activities when not observed.
//!
//! Autonomy rules loaded from `config/autonomy.ron`.

/// Simulates NPC activity off-screen using simplified tick-based logic.
pub struct AutonomySimulator {
    // TODO: NPC state snapshots, tick queue
}

impl AutonomySimulator {
    pub fn new() -> Self {
        Self {}
    }
}

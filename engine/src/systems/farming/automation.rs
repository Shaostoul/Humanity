//! Off-screen farming autonomy — simulates crop growth while player is away.
//!
//! Autonomy rules loaded from `config/farming_automation.ron`.

/// Handles off-screen farm simulation (tick-based catch-up).
pub struct FarmAutomation {
    // TODO: queued growth ticks, harvest results
}

impl FarmAutomation {
    pub fn new() -> Self {
        Self {}
    }
}

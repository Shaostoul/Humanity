//! Vehicle control — spaceships, ground vehicles, atmospheric craft.
//!
//! Vehicle definitions loaded from `data/vehicles.csv`.

pub mod ships;
pub mod propulsion;

/// Vehicle system coordinator.
pub struct VehicleSystem {
    // TODO: active vehicles, control state
}

impl VehicleSystem {
    pub fn new() -> Self {
        Self {}
    }
}

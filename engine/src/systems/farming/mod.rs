//! Farming — crop simulation, soil chemistry, and automation.
//!
//! Crop definitions loaded from `data/crops.csv`.
//! Soil parameters loaded from `data/soil_types.csv`.

pub mod crops;
pub mod soil;
pub mod automation;

/// Top-level farming simulation state.
pub struct FarmingSystem {
    // TODO: active plots, growth timers
}

impl FarmingSystem {
    pub fn new() -> Self {
        Self {}
    }
}

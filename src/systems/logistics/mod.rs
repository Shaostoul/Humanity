//! Supply chain simulation — shipping, cargo, route planning.
//!
//! Logistics parameters loaded from `config/logistics.toml`.

pub mod shipping;
pub mod cargo;

/// Supply chain simulation coordinator.
pub struct LogisticsSystem {
    // TODO: active shipments, route graph
}

impl LogisticsSystem {
    pub fn new() -> Self {
        Self {}
    }
}

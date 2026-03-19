//! Volumetric inventory system — items occupy cubic-meter volumes.
//!
//! Item definitions loaded from `data/items.csv`.

pub mod items;
pub mod containers;

/// Inventory system coordinator.
pub struct InventorySystem {
    // TODO: item registry, container registry
}

impl InventorySystem {
    pub fn new() -> Self {
        Self {}
    }
}

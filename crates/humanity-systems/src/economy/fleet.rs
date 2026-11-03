//! Fleet-wide resource pools — shared resources across a player's fleet.
//!
//! Resource definitions loaded from `data/resources.csv`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Shared resource pool across all ships in a fleet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetResources {
    /// Resource id -> quantity.
    pub resources: HashMap<String, f64>,
}

impl FleetResources {
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
        }
    }

    pub fn get(&self, resource_id: &str) -> f64 {
        self.resources.get(resource_id).copied().unwrap_or(0.0)
    }

    pub fn add(&mut self, resource_id: &str, amount: f64) {
        let entry = self.resources.entry(resource_id.to_string()).or_insert(0.0);
        *entry += amount;
    }
}

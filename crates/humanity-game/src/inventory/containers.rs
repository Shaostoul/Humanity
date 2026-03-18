//! Volumetric containers — storage with cubic-meter capacity.

use serde::{Deserialize, Serialize};

/// A container with volumetric capacity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub capacity_m3: f32,
    pub used_m3: f32,
    // TODO: Vec of item stacks
}

impl Container {
    pub fn new(capacity_m3: f32) -> Self {
        Self {
            capacity_m3,
            used_m3: 0.0,
        }
    }

    pub fn remaining_m3(&self) -> f32 {
        self.capacity_m3 - self.used_m3
    }
}

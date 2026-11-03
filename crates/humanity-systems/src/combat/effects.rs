//! Status effects with duration — buffs, debuffs, damage-over-time.
//!
//! Effect definitions loaded from `data/effects.csv`.

use serde::{Deserialize, Serialize};

/// A status effect applied to an entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusEffect {
    pub id: String,
    pub name: String,
    pub remaining_seconds: f32,
    pub is_debuff: bool,
}

impl StatusEffect {
    pub fn new(id: String, name: String, duration: f32, is_debuff: bool) -> Self {
        Self {
            id,
            name,
            remaining_seconds: duration,
            is_debuff,
        }
    }

    /// Tick the effect timer. Returns true if expired.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.remaining_seconds -= dt;
        self.remaining_seconds <= 0.0
    }
}

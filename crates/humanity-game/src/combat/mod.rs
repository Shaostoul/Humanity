//! Combat system — damage, resistances, status effects.
//!
//! Damage types and resistances loaded from `data/damage_types.csv`.

pub mod damage;
pub mod effects;

/// Combat system coordinator.
pub struct CombatSystem {
    // TODO: active combatants, effect timers
}

impl CombatSystem {
    pub fn new() -> Self {
        Self {}
    }
}

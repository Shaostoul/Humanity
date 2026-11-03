//! Damage types and resistance calculations.
//!
//! Damage type definitions loaded from `data/damage_types.csv`.

use serde::{Deserialize, Serialize};

/// Categories of damage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DamageType {
    Kinetic,
    Thermal,
    Energy,
    Chemical,
    Radiation,
}

/// A damage event applied to an entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DamageEvent {
    pub damage_type: DamageType,
    pub amount: f32,
    /// Who dealt it, for death-cause lines ("killed by a Wolf"). (v0.761)
    #[serde(default)]
    pub source_name: Option<String>,
    /// True when the player dealt it - gates loot delivery to the pack
    /// (a wolf's kill is not your loot). (v0.761)
    #[serde(default)]
    pub source_is_player: bool,
}

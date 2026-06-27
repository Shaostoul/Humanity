//! Lock TYPES registry (v0.570): the data-driven catalog of locks a door (or wall) can carry, loaded
//! from `data/blueprints/lock_types.ron`. Mirrors the `wall_materials` registry pattern exactly
//! (`include_str!` + `OnceLock` + lookup-by-id). Pure serde/data -- no renderer or persistence imports
//! -- so the headless relay build parses it without a native gate.
//!
//! A door is PASSABLE only when every `LockInstance` on it is `Unlocked` or `Broken`. This generalizes
//! the legacy single `Opening.locked: bool` (an empty lock list falls back to that bool, so every
//! existing home / test is unchanged).

use serde::{Deserialize, Serialize};

/// One entry in the lock catalog. Add a type by adding a line to `lock_types.ron`.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct LockType {
    pub id: String,
    pub name: String,
    pub category: String,
    pub interaction: LockInteraction,
    #[serde(default)]
    pub defeats: Vec<DefeatMethod>,
    /// Stops working when the home loses power (an electronic keypad/biometric); use a Crank override
    /// to still open such a door with no power.
    #[serde(default)]
    pub power_dependent: bool,
    /// A no-power emergency override (the Crank): always operable regardless of power.
    #[serde(default)]
    pub is_emergency_override: bool,
}

/// How you operate a lock at the door's control panel / the lock itself.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub enum LockInteraction {
    /// Needs a matching key item to toggle (Stage 1 stubs the possession check).
    KeyItem { item_id: String },
    /// A keypad -- E opens a code-entry prompt (Stage 1 stubs the code).
    Code { len: u8 },
    /// Just turn it (E); no key.
    Knob,
    /// Hold E to wind it open -- the no-power emergency override.
    Crank { turns: u8 },
    /// Scans the owner.
    Biometric { owner_only: bool },
    /// A generic E-toggle (the v0.567 control panel).
    Panel,
}

/// A way PAST a lock. ShootOut/BlowOpen need the future destructibility system (inert until then).
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub enum DefeatMethod {
    Lockpick { skill_min: u8, tool: String, secs: f32 },
    HackPanel { skill_min: u8, secs: f32 },
    ShootOut { hp: f32 },
    BlowOpen,
    CutPower,
}

/// Runtime state of a placed lock. Defaults `Locked` (so a door with locks starts secured).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LockState {
    #[default]
    Locked,
    Unlocked,
    Broken,
}

impl LockState {
    /// A lock that no longer blocks the door (open or destroyed).
    pub fn is_open(self) -> bool {
        matches!(self, LockState::Unlocked | LockState::Broken)
    }
}

/// The lock catalog, parsed once from the embedded RON.
pub fn lock_types() -> &'static [LockType] {
    static REG: std::sync::OnceLock<Vec<LockType>> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        const SRC: &str = include_str!("../../data/blueprints/lock_types.ron");
        match ron::from_str::<Vec<LockType>>(SRC) {
            Ok(v) => v,
            Err(e) => {
                log::error!("lock_types.ron parse error: {e}");
                Vec::new()
            }
        }
    })
}

/// Look up a lock type by its `id` (what a placed `LockInstance` stores).
pub fn lock_type(id: &str) -> Option<&'static LockType> {
    lock_types().iter().find(|t| t.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_type_registry_parses_and_has_the_core_types() {
        let types = lock_types();
        assert!(types.len() >= 5, "expected the seeded catalog, got {}", types.len());
        let key = lock_type("metal_key").expect("metal_key present");
        assert!(matches!(key.interaction, LockInteraction::KeyItem { .. }));
        assert!(!key.power_dependent, "a mechanical key lock is not power-dependent");
        let keypad = lock_type("keypad").expect("keypad present");
        assert!(matches!(keypad.interaction, LockInteraction::Code { len: 4 }));
        assert!(keypad.power_dependent, "a keypad needs power");
        let crank = lock_type("crank").expect("crank present");
        assert!(crank.is_emergency_override, "the crank is the no-power override");
        assert!(lock_type("nope").is_none());
    }

    #[test]
    fn lock_state_defaults_locked_and_open_means_passable() {
        assert_eq!(LockState::default(), LockState::Locked);
        assert!(!LockState::Locked.is_open());
        assert!(LockState::Unlocked.is_open());
        assert!(LockState::Broken.is_open());
    }
}

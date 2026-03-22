//! # core-lifeform-model
//!
//! Shared lifeform interfaces for humans and non-human species.
//! This crate is intentionally UI/network/storage agnostic.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeciesClass {
    Human,
    Livestock,
    Poultry,
    Pollinator,
    Companion,
    Wildlife,
    Aquatic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeciesProfile {
    pub class: SpeciesClass,
    pub base_metabolism_per_hour: f32,
    pub hydration_burn_per_hour: f32,
    pub stress_recovery_per_hour: f32,
}

impl SpeciesProfile {
    pub fn human_baseline() -> Self {
        Self {
            class: SpeciesClass::Human,
            base_metabolism_per_hour: 1.0,
            hydration_burn_per_hour: 1.0,
            stress_recovery_per_hour: 2.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrganState {
    /// 0.0 = destroyed, 100.0 = fully healthy
    pub integrity: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnatomyState {
    pub core_organs: Vec<OrganState>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhysiologyState {
    /// 0.0 to 100.0
    pub energy: f32,
    /// 0.0 to 100.0
    pub hydration: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CognitionState {
    /// 0.0 to 100.0 (higher means greater active task pressure)
    pub thought_load: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AffectState {
    /// 0.0 to 100.0 (higher means more stressed)
    pub stress: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillState {
    /// 0.0 to 100.0
    pub capability_index: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LifeformState {
    pub species: SpeciesProfile,
    pub anatomy: AnatomyState,
    pub physiology: PhysiologyState,
    pub cognition: CognitionState,
    pub affect: AffectState,
    pub skills: SkillState,
    pub age_hours: u64,
}

impl LifeformState {
    pub fn baseline_human() -> Self {
        Self {
            species: SpeciesProfile::human_baseline(),
            anatomy: AnatomyState {
                core_organs: vec![OrganState { integrity: 100.0 }; 5],
            },
            physiology: PhysiologyState {
                energy: 100.0,
                hydration: 100.0,
            },
            cognition: CognitionState { thought_load: 10.0 },
            affect: AffectState { stress: 10.0 },
            skills: SkillState {
                capability_index: 20.0,
            },
            age_hours: 0,
        }
    }

    pub fn capability_snapshot(&self) -> f32 {
        let physiology_factor = (self.physiology.energy + self.physiology.hydration) / 200.0;
        let stress_penalty = 1.0 - (self.affect.stress / 100.0) * 0.4;
        let organ_factor = if self.anatomy.core_organs.is_empty() {
            0.0
        } else {
            self.anatomy
                .core_organs
                .iter()
                .map(|o| o.integrity)
                .sum::<f32>()
                / (self.anatomy.core_organs.len() as f32 * 100.0)
        };

        (self.skills.capability_index * physiology_factor * stress_penalty * organ_factor)
            .clamp(0.0, 100.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TickInput {
    pub elapsed_hours: u64,
    /// 0.0 to 3.0 ; 1.0 = neutral environmental pressure
    pub environment_stress_multiplier: f32,
    /// 0.0 to 5.0 ; resource intake applied this tick
    pub food_intake_units: f32,
    /// 0.0 to 5.0 ; resource intake applied this tick
    pub water_intake_units: f32,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TickError {
    #[error("elapsed_hours must be > 0")]
    ZeroElapsed,
    #[error("invalid environment_stress_multiplier")]
    InvalidStressMultiplier,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TickOutcome {
    pub incidents: Vec<Incident>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Incident {
    DehydrationWarning,
    ExhaustionWarning,
    CriticalStress,
}

pub trait LifeformTick {
    fn tick(&mut self, input: TickInput) -> Result<TickOutcome, TickError>;
}

impl LifeformTick for LifeformState {
    fn tick(&mut self, input: TickInput) -> Result<TickOutcome, TickError> {
        if input.elapsed_hours == 0 {
            return Err(TickError::ZeroElapsed);
        }
        if !(0.0..=3.0).contains(&input.environment_stress_multiplier) {
            return Err(TickError::InvalidStressMultiplier);
        }

        let elapsed = input.elapsed_hours as f32;

        self.age_hours = self.age_hours.saturating_add(input.elapsed_hours);

        let base_energy_drain = self.species.base_metabolism_per_hour * elapsed;
        let base_hydration_drain = self.species.hydration_burn_per_hour * elapsed;

        self.physiology.energy =
            (self.physiology.energy - base_energy_drain + input.food_intake_units * 4.0)
                .clamp(0.0, 100.0);
        self.physiology.hydration =
            (self.physiology.hydration - base_hydration_drain + input.water_intake_units * 5.0)
                .clamp(0.0, 100.0);

        let stress_gain = elapsed * input.environment_stress_multiplier * 2.0;
        let stress_recovery = elapsed * self.species.stress_recovery_per_hour;
        self.affect.stress = (self.affect.stress + stress_gain - stress_recovery).clamp(0.0, 100.0);

        self.cognition.thought_load =
            (self.cognition.thought_load + input.environment_stress_multiplier - 0.5)
                .clamp(0.0, 100.0);

        let mut incidents = Vec::new();
        if self.physiology.hydration < 25.0 {
            incidents.push(Incident::DehydrationWarning);
        }
        if self.physiology.energy < 20.0 {
            incidents.push(Incident::ExhaustionWarning);
        }
        if self.affect.stress > 85.0 {
            incidents.push(Incident::CriticalStress);
        }

        Ok(TickOutcome { incidents })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_is_deterministic_for_same_state_and_input() {
        let mut a = LifeformState::baseline_human();
        let mut b = LifeformState::baseline_human();

        let input = TickInput {
            elapsed_hours: 6,
            environment_stress_multiplier: 1.2,
            food_intake_units: 0.5,
            water_intake_units: 0.25,
        };

        let out_a = a.tick(input).expect("tick should succeed");
        let out_b = b.tick(input).expect("tick should succeed");

        assert_eq!(a, b, "state diverged under same deterministic input");
        assert_eq!(out_a, out_b, "outcomes diverged under same deterministic input");
    }

    #[test]
    fn capability_snapshot_reflects_stress_and_physiology() {
        let mut s = LifeformState::baseline_human();
        let baseline = s.capability_snapshot();

        s.affect.stress = 90.0;
        s.physiology.energy = 30.0;
        s.physiology.hydration = 40.0;

        let degraded = s.capability_snapshot();
        assert!(degraded < baseline);
    }

    #[test]
    fn zero_elapsed_returns_error() {
        let mut s = LifeformState::baseline_human();
        let err = s
            .tick(TickInput {
                elapsed_hours: 0,
                environment_stress_multiplier: 1.0,
                food_intake_units: 0.0,
                water_intake_units: 0.0,
            })
            .expect_err("expected error");

        assert_eq!(err, TickError::ZeroElapsed);
    }
}

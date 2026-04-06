use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaterSourceKind {
    Rain,
    Well,
    Surface,
    Stored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Potability {
    Potable,
    NonPotable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterQuality {
    /// 0.0 (clean) to 100.0 (high contamination)
    pub contamination_index: f32,
    pub potability: Potability,
}

impl WaterQuality {
    pub fn recalculate_potability(&mut self) {
        self.potability = if self.contamination_index < 20.0 {
            Potability::Potable
        } else {
            Potability::NonPotable
        };
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaterNode {
    pub source_kind: WaterSourceKind,
    /// liters currently stored
    pub liters: f32,
    pub quality: WaterQuality,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TreatmentStep {
    /// 0.0 to 1.0 reduction factor for contamination
    pub efficacy: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DemandProfile {
    pub liters_humans: f32,
    pub liters_livestock: f32,
    pub liters_irrigation: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RiskReport {
    pub shortage_risk: f32,
    pub contamination_risk: f32,
}

#[derive(Debug, Error, PartialEq)]
pub enum WaterError {
    #[error("insufficient liters in source")]
    InsufficientWater,
    #[error("invalid treatment efficacy")]
    InvalidTreatment,
}

pub fn treat_water(node: &mut WaterNode, step: TreatmentStep) -> Result<(), WaterError> {
    if !(0.0..=1.0).contains(&step.efficacy) {
        return Err(WaterError::InvalidTreatment);
    }

    node.quality.contamination_index *= 1.0 - step.efficacy;
    node.quality.contamination_index = node.quality.contamination_index.clamp(0.0, 100.0);
    node.quality.recalculate_potability();

    Ok(())
}

pub fn route_water(from: &mut WaterNode, to: &mut WaterNode, liters: f32) -> Result<(), WaterError> {
    if from.liters < liters {
        return Err(WaterError::InsufficientWater);
    }

    from.liters -= liters;

    // Weighted quality merge into destination.
    let prior = to.liters;
    let next = prior + liters;
    if next > 0.0 {
        to.quality.contamination_index =
            (to.quality.contamination_index * prior + from.quality.contamination_index * liters) / next;
    }
    to.liters = next;
    to.quality.recalculate_potability();

    Ok(())
}

pub fn risk_report(total_liters: f32, quality: &WaterQuality, demand: DemandProfile) -> RiskReport {
    let required = demand.liters_humans + demand.liters_livestock + demand.liters_irrigation;

    let shortage_risk = if required <= 0.0 {
        0.0
    } else {
        ((required - total_liters) / required).clamp(0.0, 1.0)
    };

    let contamination_risk = (quality.contamination_index / 100.0).clamp(0.0, 1.0);

    RiskReport {
        shortage_risk,
        contamination_risk,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn treatment_reduces_contamination() {
        let mut node = WaterNode {
            source_kind: WaterSourceKind::Stored,
            liters: 100.0,
            quality: WaterQuality {
                contamination_index: 55.0,
                potability: Potability::NonPotable,
            },
        };

        treat_water(&mut node, TreatmentStep { efficacy: 0.6 }).expect("valid treatment");

        assert!(node.quality.contamination_index < 50.0);
        assert_eq!(node.quality.potability, Potability::NonPotable);

        treat_water(&mut node, TreatmentStep { efficacy: 0.7 }).expect("valid treatment");
        assert_eq!(node.quality.potability, Potability::Potable);
    }

    #[test]
    fn routing_requires_sufficient_source() {
        let mut from = WaterNode {
            source_kind: WaterSourceKind::Well,
            liters: 5.0,
            quality: WaterQuality {
                contamination_index: 5.0,
                potability: Potability::Potable,
            },
        };

        let mut to = WaterNode {
            source_kind: WaterSourceKind::Stored,
            liters: 10.0,
            quality: WaterQuality {
                contamination_index: 30.0,
                potability: Potability::NonPotable,
            },
        };

        let err = route_water(&mut from, &mut to, 10.0).expect_err("should fail");
        assert_eq!(err, WaterError::InsufficientWater);
    }

    #[test]
    fn risk_report_flags_shortage() {
        let quality = WaterQuality {
            contamination_index: 40.0,
            potability: Potability::NonPotable,
        };

        let report = risk_report(
            12.0,
            &quality,
            DemandProfile {
                liters_humans: 8.0,
                liters_livestock: 10.0,
                liters_irrigation: 10.0,
            },
        );

        assert!(report.shortage_risk > 0.0);
        assert!(report.contamination_risk > 0.0);
    }
}

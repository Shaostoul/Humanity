use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrowthStage {
    Seed,
    Sprout,
    Vegetative,
    Flowering,
    Harvestable,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CropInstance {
    pub stage: GrowthStage,
    /// 0..100
    pub vitality: f32,
    /// 0..100
    pub stress: f32,
    /// cumulative progress toward next stage
    pub growth_progress: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentInput {
    /// 0..100
    pub moisture: f32,
    /// 0..100
    pub nutrient_index: f32,
    /// 0..100
    pub temperature_suitability: f32,
    /// 0..100
    pub pollination_support: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Intervention {
    /// 0..100
    pub irrigation_boost: f32,
    /// 0..100
    pub nutrient_boost: f32,
    /// 0..100
    pub pest_control: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HarvestReport {
    pub yield_score: f32,
    pub quality_score: f32,
}

#[derive(Debug, Error, PartialEq)]
pub enum CropError {
    #[error("input out of range")]
    OutOfRange,
    #[error("crop not harvestable")]
    NotHarvestable,
}

fn validate_0_100(values: &[f32]) -> Result<(), CropError> {
    if values.iter().any(|v| !(0.0..=100.0).contains(v)) {
        return Err(CropError::OutOfRange);
    }
    Ok(())
}

pub fn apply_intervention(crop: &mut CropInstance, action: Intervention) -> Result<(), CropError> {
    validate_0_100(&[action.irrigation_boost, action.nutrient_boost, action.pest_control])?;

    let stress_reduction = (action.irrigation_boost + action.nutrient_boost + action.pest_control) / 6.0;
    crop.stress = (crop.stress - stress_reduction).clamp(0.0, 100.0);
    crop.vitality = (crop.vitality + (action.irrigation_boost + action.nutrient_boost) / 20.0).clamp(0.0, 100.0);

    Ok(())
}

pub fn tick_growth(crop: &mut CropInstance, env: EnvironmentInput) -> Result<(), CropError> {
    validate_0_100(&[
        env.moisture,
        env.nutrient_index,
        env.temperature_suitability,
        env.pollination_support,
    ])?;

    let favorable = (env.moisture + env.nutrient_index + env.temperature_suitability) / 3.0;
    let stress_gain = (100.0 - favorable) * 0.08;
    crop.stress = (crop.stress + stress_gain).clamp(0.0, 100.0);

    let vitality_delta = favorable * 0.03 - crop.stress * 0.04;
    crop.vitality = (crop.vitality + vitality_delta).clamp(0.0, 100.0);

    if crop.vitality < 8.0 {
        crop.stage = GrowthStage::Failed;
        return Ok(());
    }

    let pollination_factor = if matches!(crop.stage, GrowthStage::Flowering) {
        env.pollination_support / 100.0
    } else {
        1.0
    };

    let progress = favorable * 0.12 * pollination_factor;
    crop.growth_progress += progress;

    while crop.growth_progress >= 100.0 {
        crop.growth_progress -= 100.0;
        crop.stage = match crop.stage {
            GrowthStage::Seed => GrowthStage::Sprout,
            GrowthStage::Sprout => GrowthStage::Vegetative,
            GrowthStage::Vegetative => GrowthStage::Flowering,
            GrowthStage::Flowering => GrowthStage::Harvestable,
            GrowthStage::Harvestable => GrowthStage::Harvestable,
            GrowthStage::Failed => GrowthStage::Failed,
        };
    }

    Ok(())
}

pub fn harvest_report(crop: &CropInstance) -> Result<HarvestReport, CropError> {
    if crop.stage != GrowthStage::Harvestable {
        return Err(CropError::NotHarvestable);
    }

    let yield_score = (crop.vitality * (1.0 - crop.stress / 120.0)).clamp(0.0, 100.0);
    let quality_score = (crop.vitality * (1.0 - crop.stress / 100.0)).clamp(0.0, 100.0);

    Ok(HarvestReport {
        yield_score,
        quality_score,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline_crop() -> CropInstance {
        CropInstance {
            stage: GrowthStage::Seed,
            vitality: 60.0,
            stress: 20.0,
            growth_progress: 0.0,
        }
    }

    #[test]
    fn favorable_conditions_advance_growth() {
        let mut c = baseline_crop();
        for _ in 0..20 {
            tick_growth(
                &mut c,
                EnvironmentInput {
                    moisture: 80.0,
                    nutrient_index: 80.0,
                    temperature_suitability: 85.0,
                    pollination_support: 75.0,
                },
            )
            .expect("valid growth tick");
        }

        assert!(
            matches!(
                c.stage,
                GrowthStage::Sprout
                    | GrowthStage::Vegetative
                    | GrowthStage::Flowering
                    | GrowthStage::Harvestable
            ),
            "crop should advance beyond seed under favorable conditions; got {:?}",
            c.stage
        );
    }

    #[test]
    fn severe_conditions_can_fail_crop() {
        let mut c = baseline_crop();
        for _ in 0..60 {
            tick_growth(
                &mut c,
                EnvironmentInput {
                    moisture: 5.0,
                    nutrient_index: 5.0,
                    temperature_suitability: 10.0,
                    pollination_support: 10.0,
                },
            )
            .expect("valid growth tick");
            if c.stage == GrowthStage::Failed {
                break;
            }
        }

        assert_eq!(c.stage, GrowthStage::Failed);
    }

    #[test]
    fn harvest_requires_harvestable_stage() {
        let c = baseline_crop();
        let err = harvest_report(&c).expect_err("should reject early harvest");
        assert_eq!(err, CropError::NotHarvestable);
    }
}

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SoilTexture {
    Sand,
    Loam,
    Clay,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoilCell {
    pub texture: SoilTexture,
    /// 0..100
    pub moisture: f32,
    /// 0..100
    pub nutrient_index: f32,
    /// 0..100 (higher means worse compaction)
    pub compaction: f32,
    /// 0..100 (higher means better microbial activity)
    pub biology: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SeasonInput {
    /// 0..100
    pub rainfall: f32,
    /// 0..100
    pub heat: f32,
    /// 0..100
    pub tillage_intensity: f32,
    /// 0..100
    pub amendment_boost: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SoilTrend {
    pub fertility_delta: f32,
    pub erosion_risk: f32,
}

#[derive(Debug, Error, PartialEq)]
pub enum SoilError {
    #[error("input out of range")]
    OutOfRange,
}

fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 100.0)
}

pub fn apply_amendment(cell: &mut SoilCell, amount: f32) {
    cell.nutrient_index = clamp01(cell.nutrient_index + amount * 0.5);
    cell.biology = clamp01(cell.biology + amount * 0.4);
}

pub fn simulate_season(cell: &mut SoilCell, input: SeasonInput) -> Result<SoilTrend, SoilError> {
    let vals = [
        input.rainfall,
        input.heat,
        input.tillage_intensity,
        input.amendment_boost,
    ];
    if vals.iter().any(|v| !(0.0..=100.0).contains(v)) {
        return Err(SoilError::OutOfRange);
    }

    let moisture_gain = input.rainfall * 0.4 - input.heat * 0.25;
    cell.moisture = clamp01(cell.moisture + moisture_gain);

    let compaction_shift = input.tillage_intensity * 0.2 - input.rainfall * 0.1;
    cell.compaction = clamp01(cell.compaction + compaction_shift);

    let fertility_gain = input.amendment_boost * 0.25 + cell.biology * 0.05;
    let fertility_loss = input.heat * 0.08 + cell.compaction * 0.05;
    let delta = fertility_gain - fertility_loss;

    cell.nutrient_index = clamp01(cell.nutrient_index + delta);
    cell.biology = clamp01(cell.biology + input.amendment_boost * 0.1 - input.tillage_intensity * 0.08);

    let erosion_risk = ((input.rainfall * 0.4) + (cell.compaction * 0.4) - (cell.biology * 0.3))
        .clamp(0.0, 100.0)
        / 100.0;

    Ok(SoilTrend {
        fertility_delta: delta,
        erosion_risk,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline() -> SoilCell {
        SoilCell {
            texture: SoilTexture::Loam,
            moisture: 50.0,
            nutrient_index: 50.0,
            compaction: 30.0,
            biology: 50.0,
        }
    }

    #[test]
    fn amendment_improves_nutrients() {
        let mut cell = baseline();
        apply_amendment(&mut cell, 20.0);
        assert!(cell.nutrient_index > 50.0);
        assert!(cell.biology > 50.0);
    }

    #[test]
    fn season_simulation_produces_valid_risk() {
        let mut cell = baseline();
        let trend = simulate_season(
            &mut cell,
            SeasonInput {
                rainfall: 70.0,
                heat: 45.0,
                tillage_intensity: 20.0,
                amendment_boost: 30.0,
            },
        )
        .expect("valid inputs");

        assert!((0.0..=1.0).contains(&trend.erosion_risk));
    }

    #[test]
    fn invalid_input_is_rejected() {
        let mut cell = baseline();
        let err = simulate_season(
            &mut cell,
            SeasonInput {
                rainfall: 101.0,
                heat: 20.0,
                tillage_intensity: 20.0,
                amendment_boost: 20.0,
            },
        )
        .expect_err("should reject out-of-range input");

        assert_eq!(err, SoilError::OutOfRange);
    }
}

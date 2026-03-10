use core_session_orchestrator::FidelityPreset;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProgressionProfile {
    pub thresholds: Vec<u32>,
}

impl Default for ProgressionProfile {
    fn default() -> Self {
        Self {
            thresholds: vec![0, 100, 260, 500, 820, 1250],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillRecord {
    pub xp: u32,
    pub level: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SkillBook {
    pub skills: BTreeMap<String, SkillRecord>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProgressionError {
    #[error("skill id cannot be empty")]
    EmptySkill,
}

pub fn xp_multiplier_for_preset(preset: FidelityPreset) -> f32 {
    match preset {
        FidelityPreset::BabyCreative => 1.8,
        FidelityPreset::Easy => 1.4,
        FidelityPreset::Medium => 1.0,
        FidelityPreset::Hard => 0.85,
        FidelityPreset::Realistic => 0.7,
    }
}

pub fn level_for_xp(profile: &ProgressionProfile, xp: u32) -> u8 {
    let mut level = 0u8;
    for (idx, threshold) in profile.thresholds.iter().enumerate() {
        if xp >= *threshold {
            level = idx as u8;
        }
    }
    level
}

pub fn award_xp(
    book: &mut SkillBook,
    profile: &ProgressionProfile,
    skill_id: &str,
    base_xp: u32,
    preset: FidelityPreset,
) -> Result<SkillRecord, ProgressionError> {
    if skill_id.trim().is_empty() {
        return Err(ProgressionError::EmptySkill);
    }

    let mult = xp_multiplier_for_preset(preset);
    let gained = ((base_xp as f32) * mult).round().max(0.0) as u32;

    let rec = book.skills.entry(skill_id.to_string()).or_insert(SkillRecord { xp: 0, level: 0 });
    rec.xp = rec.xp.saturating_add(gained);
    rec.level = level_for_xp(profile, rec.xp);

    Ok(rec.clone())
}

pub fn capability_index(book: &SkillBook) -> f32 {
    if book.skills.is_empty() {
        return 0.0;
    }

    let avg_level = book.skills.values().map(|s| s.level as f32).sum::<f32>() / book.skills.len() as f32;
    let avg_xp = book.skills.values().map(|s| s.xp as f32).sum::<f32>() / book.skills.len() as f32;

    (avg_level * 10.0 + (avg_xp / 50.0)).clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn realistic_preset_grants_less_xp_than_easy() {
        let mut easy_book = SkillBook::default();
        let mut realistic_book = SkillBook::default();
        let profile = ProgressionProfile::default();

        let _ = award_xp(&mut easy_book, &profile, "carpentry", 100, FidelityPreset::Easy).unwrap();
        let _ = award_xp(
            &mut realistic_book,
            &profile,
            "carpentry",
            100,
            FidelityPreset::Realistic,
        )
        .unwrap();

        let e = easy_book.skills.get("carpentry").unwrap().xp;
        let r = realistic_book.skills.get("carpentry").unwrap().xp;
        assert!(e > r);
    }

    #[test]
    fn level_progression_crosses_thresholds() {
        let mut book = SkillBook::default();
        let profile = ProgressionProfile::default();

        let rec = award_xp(&mut book, &profile, "water", 300, FidelityPreset::Medium).unwrap();
        assert!(rec.level >= 2);
    }
}

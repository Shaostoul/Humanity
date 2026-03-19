use core_session_orchestrator::FidelityPreset;
use core_skill_progression::{award_xp, capability_index, ProgressionProfile, SkillBook};

fn main() {
    let mut book = SkillBook::default();
    let profile = ProgressionProfile::default();

    let _ = award_xp(&mut book, &profile, "carpentry", 120, FidelityPreset::Medium).unwrap();
    let _ = award_xp(&mut book, &profile, "water", 90, FidelityPreset::Hard).unwrap();

    println!("skills={:?}", book.skills);
    println!("capability_index={:.2}", capability_index(&book));
}

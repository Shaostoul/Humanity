use core_skill_progression::{award_xp, ProgressionProfile, SkillBook};
use core_session_orchestrator::FidelityPreset;
use core_teaching_graph::{
    add_node, add_prereq, recommend_next, CompetencyGraph, CompetencyNode,
};

fn main() {
    let mut graph = CompetencyGraph::default();
    add_node(&mut graph, CompetencyNode { id: "water".into(), label: "Water Safety".into(), target_level: 2 }).unwrap();
    add_node(&mut graph, CompetencyNode { id: "plumbing".into(), label: "Plumbing".into(), target_level: 2 }).unwrap();
    add_prereq(&mut graph, "plumbing", "water").unwrap();

    let mut skills = SkillBook::default();
    let profile = ProgressionProfile::default();
    let _ = award_xp(&mut skills, &profile, "water", 320, FidelityPreset::Medium).unwrap();

    let recs = recommend_next(&graph, &skills, 3).unwrap();
    println!("recommendations={:?}", recs);
}

use core_skill_progression::SkillBook;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompetencyNode {
    pub id: String,
    pub label: String,
    /// target level to consider competency "mastered enough"
    pub target_level: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompetencyGraph {
    pub nodes: BTreeMap<String, CompetencyNode>,
    pub prereqs: BTreeMap<String, BTreeSet<String>>,
}

impl Default for CompetencyGraph {
    fn default() -> Self {
        Self {
            nodes: BTreeMap::new(),
            prereqs: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GraphError {
    #[error("node already exists")]
    DuplicateNode,
    #[error("node missing")]
    MissingNode,
    #[error("graph contains cycle")]
    CycleDetected,
}

pub fn add_node(graph: &mut CompetencyGraph, node: CompetencyNode) -> Result<(), GraphError> {
    if graph.nodes.contains_key(&node.id) {
        return Err(GraphError::DuplicateNode);
    }
    graph.prereqs.entry(node.id.clone()).or_default();
    graph.nodes.insert(node.id.clone(), node);
    Ok(())
}

pub fn add_prereq(graph: &mut CompetencyGraph, node: &str, prereq: &str) -> Result<(), GraphError> {
    if !graph.nodes.contains_key(node) || !graph.nodes.contains_key(prereq) {
        return Err(GraphError::MissingNode);
    }
    graph.prereqs.entry(node.to_string()).or_default().insert(prereq.to_string());
    Ok(())
}

pub fn validate_graph(graph: &CompetencyGraph) -> Result<(), GraphError> {
    let mut indegree: BTreeMap<String, usize> = graph.nodes.keys().map(|k| (k.clone(), 0)).collect();
    let mut outgoing: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for (node, reqs) in &graph.prereqs {
        for req in reqs {
            *indegree.get_mut(node).ok_or(GraphError::MissingNode)? += 1;
            outgoing.entry(req.clone()).or_default().push(node.clone());
        }
    }

    let mut q: VecDeque<String> = indegree
        .iter()
        .filter_map(|(n, d)| if *d == 0 { Some(n.clone()) } else { None })
        .collect();

    let mut visited = 0usize;
    while let Some(n) = q.pop_front() {
        visited += 1;
        if let Some(children) = outgoing.get(&n) {
            for child in children {
                let d = indegree.get_mut(child).ok_or(GraphError::MissingNode)?;
                *d -= 1;
                if *d == 0 {
                    q.push_back(child.clone());
                }
            }
        }
    }

    if visited != graph.nodes.len() {
        return Err(GraphError::CycleDetected);
    }

    Ok(())
}

pub fn unmet_prerequisites(graph: &CompetencyGraph, skill_book: &SkillBook, node_id: &str) -> Result<Vec<String>, GraphError> {
    if !graph.nodes.contains_key(node_id) {
        return Err(GraphError::MissingNode);
    }

    let reqs = graph.prereqs.get(node_id).cloned().unwrap_or_default();
    let mut unmet = Vec::new();
    for req in reqs {
        let target = graph.nodes.get(&req).ok_or(GraphError::MissingNode)?.target_level;
        let current = skill_book.skills.get(&req).map(|s| s.level).unwrap_or(0);
        if current < target {
            unmet.push(req);
        }
    }
    unmet.sort();
    Ok(unmet)
}

pub fn recommend_next(graph: &CompetencyGraph, skill_book: &SkillBook, limit: usize) -> Result<Vec<String>, GraphError> {
    validate_graph(graph)?;

    let mut scored: Vec<(String, i32)> = Vec::new();

    for (id, node) in &graph.nodes {
        let unmet = unmet_prerequisites(graph, skill_book, id)?;
        if !unmet.is_empty() {
            continue;
        }

        let level = skill_book.skills.get(id).map(|s| s.level).unwrap_or(0);
        if level >= node.target_level {
            continue;
        }

        let gap = node.target_level as i32 - level as i32;
        let score = gap * 100 - graph.prereqs.get(id).map(|p| p.len() as i32).unwrap_or(0);
        scored.push((id.clone(), score));
    }

    scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    Ok(scored.into_iter().take(limit).map(|x| x.0).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_skill_progression::{ProgressionProfile, SkillBook, award_xp};
    use core_session_orchestrator::FidelityPreset;

    fn sample_graph() -> CompetencyGraph {
        let mut g = CompetencyGraph::default();
        add_node(&mut g, CompetencyNode { id: "water".into(), label: "Water Safety".into(), target_level: 2 }).unwrap();
        add_node(&mut g, CompetencyNode { id: "plumbing".into(), label: "Plumbing Basics".into(), target_level: 2 }).unwrap();
        add_node(&mut g, CompetencyNode { id: "farm".into(), label: "Crop Systems".into(), target_level: 2 }).unwrap();
        add_prereq(&mut g, "plumbing", "water").unwrap();
        g
    }

    #[test]
    fn cycle_detection_works() {
        let mut g = sample_graph();
        add_prereq(&mut g, "water", "plumbing").unwrap();
        assert_eq!(validate_graph(&g).unwrap_err(), GraphError::CycleDetected);
    }

    #[test]
    fn recommendations_respect_prereqs() {
        let g = sample_graph();
        let book = SkillBook::default();
        let recs = recommend_next(&g, &book, 5).unwrap();
        assert!(recs.contains(&"water".to_string()));
        assert!(!recs.contains(&"plumbing".to_string()));
    }

    #[test]
    fn unlocked_skill_appears_after_prereq_progress() {
        let g = sample_graph();
        let mut book = SkillBook::default();
        let profile = ProgressionProfile::default();
        let _ = award_xp(&mut book, &profile, "water", 500, FidelityPreset::Easy).unwrap();

        let recs = recommend_next(&g, &book, 5).unwrap();
        assert!(recs.contains(&"plumbing".to_string()));
    }
}

//! Load-bearing analysis — validates structural integrity of builds.
//!
//! Material strength properties loaded from `data/materials.csv`.

/// Result of a structural integrity check.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StructuralResult {
    Stable,
    Unstable,
    Collapsed,
}

impl From<super::solver::StructuralVerdict> for StructuralResult {
    fn from(v: super::solver::StructuralVerdict) -> Self {
        match v {
            super::solver::StructuralVerdict::Stable => StructuralResult::Stable,
            super::solver::StructuralVerdict::Unstable => StructuralResult::Unstable,
            super::solver::StructuralVerdict::Collapsed => StructuralResult::Collapsed,
        }
    }
}

/// Analyzes load-bearing capacity of a construction.
pub struct StructuralAnalyzer;

impl StructuralAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Run the node-beam solver (see `super::solver`) and reduce it to the three-state verdict.
    /// The de-risk-spike entry point: hand it a framing graph, get Stable / Unstable / Collapsed.
    pub fn analyze(
        &self,
        nodes: &[super::solver::FramingNode],
        members: &[super::solver::FramingMember],
    ) -> StructuralResult {
        super::solver::solve(nodes, members).verdict.into()
    }
}

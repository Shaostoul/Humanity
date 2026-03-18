//! Data-driven behavior trees for NPC decision-making.
//!
//! Tree definitions loaded from `data/behaviors.ron`.

use serde::{Deserialize, Serialize};

/// Result of a behavior tree node tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BehaviorStatus {
    Success,
    Failure,
    Running,
}

/// A behavior tree node (data-driven, loaded from RON).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BehaviorNode {
    /// Run children in order until one fails.
    Sequence(Vec<BehaviorNode>),
    /// Run children in order until one succeeds.
    Selector(Vec<BehaviorNode>),
    /// A leaf action identified by name.
    Action(String),
    /// A leaf condition check identified by name.
    Condition(String),
}

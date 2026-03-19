//! Quest engine — objective tracking and quest state machine.
//!
//! Quest definitions loaded from `data/quests.ron`.

pub mod objectives;

/// Manages active and completed quests.
pub struct QuestEngine {
    // TODO: active quests, completed set
}

impl QuestEngine {
    pub fn new() -> Self {
        Self {}
    }
}

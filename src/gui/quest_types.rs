//! The Quests page's value types (moved out of `state_types.rs` 2026-10-30, when a quest card
//! gained its chain, its id and its rewards for the chain window, and that file was at its size
//! budget). `pub use quest_types::*` in the parent keeps the `crate::gui::GuiQuest` spelling.
//!
//! The constructors are the quest mirror's (lib.rs builds `GuiState::quests` from the ECS
//! QuestTracker every frame): they live here, beside the type, so lib.rs stays the frame's
//! wiring and not its arithmetic.

use crate::systems::quests::{chain, ActiveQuest, QuestRegistry};

/// A player quest for GUI display, synced from the ECS QuestTracker each frame.
/// Active quests carry their current step (index/total + description), the chain they belong
/// to and their rewards; completed quests have `completed = true` and `step_total = 0`.
#[derive(Debug, Clone, Default)]
pub struct GuiQuest {
    /// The quest's id: what a click on its card opens the chain window for.
    pub id: String,
    pub name: String,
    pub step_index: usize,
    pub step_total: usize,
    pub step_desc: String,
    pub completed: bool,
    /// The name of the quest at the top of its chain (its own name when it is the top).
    pub chain_name: String,
    /// Where it stands in that chain, counting from 1 at the top.
    pub chain_pos: usize,
    /// Item rewards by name, and how many.
    pub rewards: Vec<(String, u32)>,
    /// Skill XP rewards by skill name, and how much.
    pub xp_rewards: Vec<(String, u32)>,
}

impl GuiQuest {
    /// A quest under way, as its card shows it: the current step as the player reads it
    /// (`words.step`, the own front door named; `chain::with_store_words` in the game), its
    /// chain and its rewards.
    pub fn active(registry: Option<&QuestRegistry>, active: &ActiveQuest, words: &chain::Words) -> Self {
        let def = registry.and_then(|r| r.get(&active.quest_id));
        let (chain_name, chain_pos) = registry
            .and_then(|r| chain::top_of(r, &active.quest_id))
            .map_or_else(|| (String::new(), 1), |(top, depth)| (top.name.clone(), depth + 1));
        let (rewards, xp_rewards) = def.map(|d| chain::named_rewards(d, words)).unwrap_or_default();
        GuiQuest {
            id: active.quest_id.clone(),
            name: def.map_or_else(|| active.quest_id.clone(), |d| d.name.clone()),
            step_index: active.current_step,
            step_total: def.map_or(0, |d| d.steps.len()),
            step_desc: def.and_then(|d| d.steps.get(active.current_step)).map(|s| (words.step)(&s.description)).unwrap_or_default(),
            completed: false,
            chain_name,
            chain_pos,
            rewards,
            xp_rewards,
        }
    }

    /// A finished quest: its name, and its id for the chain window.
    pub fn completed(registry: Option<&QuestRegistry>, id: &str) -> Self {
        GuiQuest {
            id: id.to_string(),
            name: registry.and_then(|r| r.get(id)).map_or_else(|| id.to_string(), |d| d.name.clone()),
            completed: true,
            chain_pos: 1,
            ..Default::default()
        }
    }
}

/// A quest the player COULD accept (v0.747.x, ladder rung 4): prerequisite
/// satisfied, not active, not completed. The Quests page's Available section.
#[derive(Debug, Clone)]
pub struct GuiAvailableQuest {
    pub id: String,
    pub name: String,
    pub description: String,
}

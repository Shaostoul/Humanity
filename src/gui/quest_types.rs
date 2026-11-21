//! The Quests page's value types (moved out of `state_types.rs` 2026-10-30, when a quest card
//! gained its chain, its id and its rewards for the chain window, and that file was at its size
//! budget). `pub use quest_types::*` in the parent keeps the `crate::gui::GuiQuest` spelling.
//!
//! The constructors are the quest mirror's (lib.rs builds `GuiState::quests` from the ECS
//! QuestTracker every frame): they live here, beside the type, so lib.rs stays the frame's
//! wiring and not its arithmetic.

use humanity_systems::quests::{chain, ActiveQuest, QuestRegistry};

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
    /// The one the HUD shows (P2: the tracked quest, else the most recently accepted;
    /// `QuestTracker::hud_quest`).
    pub tracked: bool,
    /// For a Keep step, the reading now, its band and how long it has held (`keep_line`), shown
    /// under the step on its card: the meter to watch.
    pub keep_line: Option<String>,
}

/// A Keep step's line for its card (2026-11-20, E2; docs/design/quest-chains.md 2.10: "so the
/// Quests page can show 'Hydration 62, keep it at 60 or more'"): the reading now, in its unit, the
/// band, and for a step held over hours, how many have held. None for any other step, or a
/// reading the body does not have.
pub fn keep_line(
    objective: &humanity_systems::quests::QuestObjective,
    step: usize,
    progress: &std::collections::HashMap<String, u32>,
    body: &humanity_systems::quests::readings::Body,
) -> Option<String> {
    use humanity_core::i18n::{t, t_dyn, tf};
    use humanity_systems::quests::{readings, QuestObjective};
    let QuestObjective::Keep { reading, min, max, hours, .. } = objective else { return None };
    let value = readings::read(reading, body)?;
    let unit = readings::unit_key(reading).map(t).unwrap_or_default();
    let num = |v: f32| if v.abs() >= 100.0 || v.fract().abs() < 0.05 { format!("{v:.0}") } else { format!("{v:.1}") };
    let label = t_dyn(&readings::label_key(reading));
    let now = tf("quests.keep_now", &[("reading", &label), ("value", &num(value)), ("unit", &unit)]);
    let band = match (min, max) {
        (Some(lo), None) => tf("quests.keep_min", &[("value", &num(*lo)), ("unit", &unit)]),
        (None, Some(hi)) => tf("quests.keep_max", &[("value", &num(*hi)), ("unit", &unit)]),
        (Some(lo), Some(hi)) => tf("quests.keep_between", &[("low", &num(*lo)), ("high", &num(*hi)), ("unit", &unit)]),
        (None, None) => String::new(),
    };
    let mut line = format!("{now} {band}").trim().to_string();
    if *hours > 0.0 {
        let held = progress
            .get(&humanity_systems::quests::keep_since_key(step))
            .map_or(0.0, |since| ((body.now_s - f64::from(*since)).max(0.0) / 3600.0) as f32);
        line.push(' ');
        line.push_str(&tf("quests.keep_held", &[("held", &format!("{:.0}", held.floor().min(*hours))), ("hours", &num(*hours))]));
    }
    Some(line)
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
            tracked: false,
            keep_line: None,
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

/// A quest the player COULD accept (v0.747.x, ladder rung 4): its `after`
/// met, not active, not completed. The Quests page's Available section.
#[derive(Debug, Clone)]
pub struct GuiAvailableQuest {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[cfg(test)]
mod tests {
    use super::keep_line;
    use humanity_systems::quests::{keep_since_key, readings::Body, QuestObjective};

    /// A Keep step's card line fills every word it names (2026-11-21): the reading, its value and
    /// unit, the band, and the hours held, with no placeholder left for a person to read.
    /// Seen red 2026-11-21 with the words as rung 1 wrote them, single braces where the word
    /// source fills double (`i18n::fill`): the GPU snapshot of the Quests page read "{reading} is
    /// {value} {unit} now; keep it at {value} {unit} or more. Held for {held} of {hours} hours.",
    /// and this test: "a placeholder was left: {reading} is {value} {unit} now; keep it at {value}
    /// {unit} or more. Held for {held} of {hours} hours."
    #[test]
    fn a_keep_steps_line_fills_every_word() {
        let vitals = humanity_core::ecs::components::Vitals { hydration: 72.0, ..Default::default() };
        let now = 30.0 * 3600.0;
        let body = Body { vitals: Some(&vitals), intake: None, now_s: now };
        let keep = QuestObjective::Keep { reading: "vital.hydration".into(), min: Some(60.0), max: None, hours: 24.0, at: None };
        let mut progress = std::collections::HashMap::new();
        progress.insert(keep_since_key(0), (now - 5.0 * 3600.0) as u32);
        let line = keep_line(&keep, 0, &progress, &body).expect("a Keep step on a reading the body has");
        assert!(!line.contains('{') && !line.contains('}'), "a placeholder was left: {line}");
        assert!(line.contains("72") && line.contains("60") && line.contains("5 of 24"), "the reading, the band and the hours held: {line}");
    }
}

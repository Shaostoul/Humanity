//! A quest's CHAIN, as the Quests page's chain window shows it (operator, 2026-10-30: "when we
//! click on that quest, I want it to like popup all the quests in the chain. We'd want two
//! modes. One mode where we see all of the quest steps for dev. Then normal play shows only
//! what's been learned. Like we shouldn't know what step 8 is at step 1").
//!
//! A chain is every quest joined to the clicked one by `prerequisite` links: the quest at the
//! top (no prerequisite), and every quest that needs it finished, and every quest that needs
//! those, and so on. A quest has at most one prerequisite, so a chain is a TREE: two or more
//! quests needing the same one are BRANCHES, and finishing that one starts them all
//! (`QuestSystem`'s chaining), so no branch ever locks another.
//!
//! What a chain shows depends on [`Reveal`]:
//! - `Learned` (normal play): only what the player has reached. Finished quests with their
//!   steps, a quest under way with its finished steps and the step it is on, a quest the player
//!   could accept with its name. The steps after the current one and the quests not unlocked
//!   yet are LEFT OUT of the view, not just hidden on screen, so nothing drawing it can spoil
//!   them: all that remains is how many steps are left ([`ChainQuest::hidden_steps`]) and
//!   whether more quests follow ([`ChainQuest::hidden_next`]).
//! - `All` (the Dev play mode with dev cheats on, `GuiState::dev_cheats_active`): every quest
//!   and every step, each marked with how far the player has got, so the page can dim the ones
//!   not reached yet.
//!
//! Pure: it reads a registry and a tracker and turns words through [`Words`], so it is tested
//! on its own (below, and on the shipped quests) and built only while a chain window is open
//! (the quest mirror in lib.rs).

use std::collections::HashSet;

use super::{QuestDef, QuestRegistry, QuestTracker};

/// How much of a chain the player is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reveal {
    /// Normal play: only what the player has reached.
    Learned,
    /// Dev: everything, each part marked with how far the player has got.
    All,
}

/// Where the player is with a quest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestState {
    /// Finished.
    Done,
    /// Under way.
    Active,
    /// Not started, but nothing stands in its way: no prerequisite, or a finished one.
    Available,
    /// Its prerequisite is not finished yet. Only an `All` view holds one.
    Locked,
}

/// Where the player is with one step of a quest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Done,
    /// The step the player is on now.
    Current,
    /// Not reached yet. Only an `All` view holds one.
    Ahead,
}

/// One step as the chain window shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct ChainStep {
    /// What to do, as the player reads it (`step_text`).
    pub text: String,
    pub state: StepState,
}

/// One quest of a chain, with the quests after it.
#[derive(Debug, Clone, PartialEq)]
pub struct ChainQuest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub state: QuestState,
    /// The steps shown: every step in an `All` view; in a `Learned` one the finished steps and
    /// the current one (none for a quest not started).
    pub steps: Vec<ChainStep>,
    /// How many steps the quest has in all.
    pub step_total: usize,
    /// Item rewards by name, and how many.
    pub rewards: Vec<(String, u32)>,
    /// Skill XP rewards by skill name, and how much.
    pub xp_rewards: Vec<(String, u32)>,
    /// The quests that need this one finished, as shown, by name. Two or more are branches.
    pub next: Vec<ChainQuest>,
    /// How many of the quests that need this one are left out (a `Learned` view: not unlocked).
    pub hidden_next: usize,
}

impl ChainQuest {
    /// How many of its steps are not shown.
    pub fn hidden_steps(&self) -> usize {
        self.step_total.saturating_sub(self.steps.len())
    }
}

/// A whole chain, from the quest at its top.
#[derive(Debug, Clone, PartialEq)]
pub struct ChainView {
    /// The quest the player clicked, which the window points out.
    pub focus: String,
    pub reveal: Reveal,
    pub root: ChainQuest,
}

impl ChainView {
    /// Every quest in the view, top first, depth first.
    pub fn quests(&self) -> Vec<&ChainQuest> {
        fn walk<'a>(q: &'a ChainQuest, out: &mut Vec<&'a ChainQuest>) {
            out.push(q);
            for n in &q.next {
                walk(n, out);
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &mut out);
        out
    }
}

/// How the view turns ids and step descriptions into the words a player reads.
pub struct Words<'a> {
    /// A step's description as the player reads it (`step_text`).
    pub step: &'a dyn Fn(&str) -> String,
    /// An item's name from its id.
    pub item: &'a dyn Fn(&str) -> String,
    /// A skill's name from its id.
    pub skill: &'a dyn Fn(&str) -> String,
}

/// The words as the game has them: steps through `step_text`, items and skills by their
/// registries' names, falling back to the id (the way `completion_notice` names them).
pub fn with_store_words<R>(data: &crate::hot_reload::data_store::DataStore, f: impl FnOnce(&Words) -> R) -> R {
    let items = data.get::<crate::systems::inventory::ItemRegistry>("item_registry");
    let skills = data.get::<crate::systems::skills::SkillRegistry>("skill_registry");
    let step = |d: &str| super::step_text(d, data);
    let item = |id: &str| items.and_then(|r| r.items.get(id)).map(|d| d.name.clone()).unwrap_or_else(|| id.to_string());
    let skill = |id: &str| skills.and_then(|r| r.get(id)).map(|d| d.name.clone()).unwrap_or_else(|| id.replace('_', " "));
    f(&Words { step: &step, item: &item, skill: &skill })
}

/// A quest's rewards by name: (items, skill XP).
pub fn named_rewards(def: &QuestDef, words: &Words) -> (Vec<(String, u32)>, Vec<(String, u32)>) {
    (
        def.rewards.iter().map(|(id, n)| ((words.item)(id), *n)).collect(),
        def.xp_rewards.iter().map(|(id, n)| ((words.skill)(id), *n)).collect(),
    )
}

/// Where the player is with a quest.
pub fn state_of(def: &QuestDef, tracker: &QuestTracker) -> QuestState {
    if tracker.is_completed(&def.id) {
        QuestState::Done
    } else if tracker.is_active(&def.id) {
        QuestState::Active
    } else if def.prerequisite.as_deref().map_or(true, |p| tracker.is_completed(p)) {
        QuestState::Available
    } else {
        QuestState::Locked
    }
}

/// The quest at the top of `id`'s chain and how many quests lead to `id` (0 when `id` is at
/// the top): follows `prerequisite` up, stopping at a quest with none, one naming a quest the
/// registry lacks, or a loop. None when the registry lacks `id`.
pub fn top_of<'a>(registry: &'a QuestRegistry, id: &str) -> Option<(&'a QuestDef, usize)> {
    let mut at = registry.get(id)?;
    let mut seen: HashSet<&str> = HashSet::new();
    seen.insert(at.id.as_str());
    let mut depth = 0;
    while let Some(up) = at.prerequisite.as_deref().and_then(|p| registry.get(p)) {
        if !seen.insert(up.id.as_str()) {
            break;
        }
        at = up;
        depth += 1;
    }
    Some((at, depth))
}

/// The chain `focus` belongs to, as `reveal` shows it. None when the registry lacks `focus`,
/// or in a `Learned` view when the player has not reached it.
pub fn build(registry: &QuestRegistry, tracker: &QuestTracker, focus: &str, reveal: Reveal, words: &Words) -> Option<ChainView> {
    let focus_def = registry.get(focus)?;
    let known = |d: &QuestDef| reveal == Reveal::All || state_of(d, tracker) != QuestState::Locked;
    if !known(focus_def) {
        return None;
    }
    // The top, as far up as the player knows: in consistent saves every quest above a known
    // one is finished, but a hand-edited save could hold one that is not, and its name stays
    // out of a Learned view like any other locked quest.
    let mut top = focus_def;
    let mut seen: HashSet<&str> = HashSet::new();
    seen.insert(top.id.as_str());
    while let Some(up) = top.prerequisite.as_deref().and_then(|p| registry.get(p)) {
        if !known(up) || !seen.insert(up.id.as_str()) {
            break;
        }
        top = up;
    }
    // Who needs whom, sorted by name so the branches keep their places (the registry is a map).
    let mut children: std::collections::HashMap<&str, Vec<&QuestDef>> = std::collections::HashMap::new();
    for d in registry.quests.values() {
        if let Some(p) = d.prerequisite.as_deref() {
            children.entry(p).or_default().push(d);
        }
    }
    for list in children.values_mut() {
        list.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    }
    let mut visited: HashSet<String> = HashSet::new();
    let root = node(top, tracker, reveal, words, &children, &mut visited);
    Some(ChainView { focus: focus.to_string(), reveal, root })
}

fn node(
    def: &QuestDef,
    tracker: &QuestTracker,
    reveal: Reveal,
    words: &Words,
    children: &std::collections::HashMap<&str, Vec<&QuestDef>>,
    visited: &mut HashSet<String>,
) -> ChainQuest {
    visited.insert(def.id.clone());
    let state = state_of(def, tracker);
    // How many steps are finished: all of a finished quest's, those before the current one of
    // a quest under way, none of one not started.
    let done = match state {
        QuestState::Done => def.steps.len(),
        QuestState::Active => tracker
            .active_quests
            .iter()
            .find(|a| a.quest_id == def.id)
            .map_or(0, |a| a.current_step.min(def.steps.len())),
        QuestState::Available | QuestState::Locked => 0,
    };
    let shown = match (reveal, state) {
        (Reveal::All, _) => def.steps.len(),
        (Reveal::Learned, QuestState::Done) => def.steps.len(),
        // The finished steps and the one the player is on: never one after it.
        (Reveal::Learned, QuestState::Active) => (done + 1).min(def.steps.len()),
        (Reveal::Learned, QuestState::Available | QuestState::Locked) => 0,
    };
    let steps = def.steps[..shown]
        .iter()
        .enumerate()
        .map(|(i, s)| ChainStep {
            text: (words.step)(&s.description),
            state: if i < done {
                StepState::Done
            } else if i == done && state == QuestState::Active {
                StepState::Current
            } else {
                StepState::Ahead
            },
        })
        .collect();
    let (rewards, xp_rewards) = named_rewards(def, words);
    let mut next = Vec::new();
    let mut hidden_next = 0;
    for child in children.get(def.id.as_str()).map(|v| v.as_slice()).unwrap_or(&[]) {
        if visited.contains(&child.id) {
            continue; // a loop in the data: each quest once
        }
        if reveal == Reveal::All || state_of(child, tracker) != QuestState::Locked {
            next.push(node(child, tracker, reveal, words, children, visited));
        } else {
            hidden_next += 1;
        }
    }
    ChainQuest {
        id: def.id.clone(),
        name: def.name.clone(),
        description: def.description.clone(),
        state,
        steps,
        step_total: def.steps.len(),
        rewards,
        xp_rewards,
        next,
        hidden_next,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::quests::{ActiveQuest, QuestObjective, QuestStep};

    fn plain_words<R>(f: impl FnOnce(&Words) -> R) -> R {
        let same = |s: &str| s.to_string();
        f(&Words { step: &same, item: &same, skill: &same })
    }

    /// A quest with `steps` steps, each step's words `<id> step <n>`.
    fn quest(id: &str, steps: usize, prereq: Option<&str>) -> QuestDef {
        QuestDef {
            id: id.to_string(),
            name: format!("{id} name"),
            description: format!("{id} description"),
            steps: (1..=steps)
                .map(|n| QuestStep { description: format!("{id} step {n}"), objective: QuestObjective::View { view: "vitals".into() } })
                .collect(),
            rewards: vec![(format!("{id}_reward"), 1)],
            xp_rewards: vec![(format!("{id}_skill"), 5)],
            prerequisite: prereq.map(str::to_string),
        }
    }

    /// Every word a view holds: names, descriptions, steps and rewards.
    fn all_words(view: &ChainView) -> Vec<String> {
        let mut out = Vec::new();
        for q in view.quests() {
            out.push(q.id.clone());
            out.push(q.name.clone());
            out.push(q.description.clone());
            out.extend(q.steps.iter().map(|s| s.text.clone()));
            out.extend(q.rewards.iter().map(|r| r.0.clone()));
            out.extend(q.xp_rewards.iter().map(|r| r.0.clone()));
        }
        out
    }

    /// a -> b -> {c, d}, d -> e: the player finished a and is on b's second step of three.
    fn mid_chain() -> (QuestRegistry, QuestTracker) {
        let mut reg = QuestRegistry::default();
        for q in [quest("a", 2, None), quest("b", 3, Some("a")), quest("c", 2, Some("b")), quest("d", 2, Some("b")), quest("e", 1, Some("d"))] {
            reg.quests.insert(q.id.clone(), q);
        }
        let tracker = QuestTracker {
            active_quests: vec![ActiveQuest { quest_id: "b".into(), current_step: 1, progress: Default::default() }],
            completed_quests: vec!["a".into()],
        };
        (reg, tracker)
    }

    /// NORMAL PLAY NEVER SPOILS (operator, 2026-10-30: "we shouldn't know what step 8 is at step
    /// 1"). Mid-chain, a Learned view holds the finished quest, the finished step and the current
    /// one, and NOTHING of the step after it or of the quests not unlocked: not their names, not
    /// their steps, not their rewards. Only how many steps are left and that more quests follow.
    ///
    /// Seen red 2026-10-30 with the Learned arm for a quest under way changed to show every step
    /// (`def.steps.len()`): "b step 3" was in the view.
    #[test]
    fn a_learned_view_never_holds_a_step_or_quest_not_reached() {
        let (reg, tracker) = mid_chain();
        let view = plain_words(|w| build(&reg, &tracker, "b", Reveal::Learned, w)).expect("b is under way");
        let words = all_words(&view);
        for secret in ["b step 3", "c name", "c step", "c_reward", "d name", "d step", "d_reward", "e name", "e step", "e_reward", "c description", "d description"] {
            assert!(
                !words.iter().any(|w| w.contains(secret)),
                "the Learned view holds {secret:?}, which the player has not reached: {words:?}"
            );
        }
        // What the player HAS reached is all there, so the test above cannot pass on an empty view.
        assert_eq!(view.root.id, "a");
        assert_eq!(view.root.state, QuestState::Done);
        assert_eq!(view.root.steps.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(), ["a step 1", "a step 2"]);
        let b = &view.root.next[0];
        assert_eq!((b.id.as_str(), b.state), ("b", QuestState::Active));
        assert_eq!(
            b.steps.iter().map(|s| (s.text.as_str(), s.state)).collect::<Vec<_>>(),
            [("b step 1", StepState::Done), ("b step 2", StepState::Current)]
        );
        assert_eq!(b.hidden_steps(), 1, "one step left, counted, not named");
        assert!(b.next.is_empty(), "c and d are not unlocked");
        assert_eq!(b.hidden_next, 2, "two quests follow b, counted, not named");
        assert_eq!(view.quests().len(), 2);
    }

    /// At every step of every quest of a chain, a Learned view never holds a step after the
    /// current one: walked over the shipped opening (gs_first_steps, eight steps, then
    /// Toolsmith and the habitat's four branches).
    #[test]
    fn the_shipped_opening_never_spoils_a_step_ahead() {
        let reg = QuestRegistry::from_ron_dir(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/quests"));
        let first = reg.get("gs_first_steps").expect("the shipped opening");
        assert!(first.steps.len() >= 2);
        for at in 0..first.steps.len() {
            let tracker = QuestTracker {
                active_quests: vec![ActiveQuest { quest_id: "gs_first_steps".into(), current_step: at, progress: Default::default() }],
                completed_quests: vec![],
            };
            let view = plain_words(|w| build(&reg, &tracker, "gs_first_steps", Reveal::Learned, w)).expect("under way");
            let words = all_words(&view);
            for later in &first.steps[at + 1..] {
                assert!(!words.contains(&later.description), "at step {} the view names a later step: {:?}", at + 1, later.description);
            }
            assert_eq!(view.root.steps.len(), at + 1, "the finished steps and the current one");
            assert_eq!(view.quests().len(), 1, "no quest after the opening is unlocked yet: {words:?}");
            assert!(view.root.hidden_next >= 1, "but the window can say more follow");
            for q in reg.quests.values().filter(|q| q.id != "gs_first_steps") {
                assert!(!words.contains(&q.name), "the view names {:?}, not unlocked", q.name);
            }
        }
    }

    /// BRANCHES (operator default, 2026-10-30): finishing a quest starts every quest that needs
    /// it, so the habitat's branches are all under way together and all shown, none locking
    /// another, while what comes after each stays hidden.
    #[test]
    fn branches_are_shown_side_by_side_and_their_followers_stay_hidden() {
        let reg = QuestRegistry::from_ron_dir(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/quests"));
        let branches: Vec<&QuestDef> = reg.quests.values().filter(|q| q.prerequisite.as_deref() == Some("tutorial_first_habitat")).collect();
        assert!(branches.len() >= 2, "the habitat has branches");
        let tracker = QuestTracker {
            active_quests: branches.iter().map(|q| ActiveQuest { quest_id: q.id.clone(), current_step: 0, progress: Default::default() }).collect(),
            completed_quests: vec!["gs_first_steps".into(), "gs_toolsmith".into(), "tutorial_first_habitat".into()],
        };
        let view = plain_words(|w| build(&reg, &tracker, &branches[0].id, Reveal::Learned, w)).expect("under way");
        assert_eq!(view.root.id, "gs_first_steps", "the chain starts at the top");
        let habitat = view.quests().into_iter().find(|q| q.id == "tutorial_first_habitat").expect("the habitat is in the chain");
        assert_eq!(habitat.next.len(), branches.len(), "every branch is shown");
        assert!(habitat.next.iter().all(|b| b.state == QuestState::Active));
        for b in &habitat.next {
            assert!(b.next.is_empty(), "{} is shown, its followers are not", b.id);
            let followers = reg.quests.values().filter(|q| q.prerequisite.as_deref() == Some(b.id.as_str())).count();
            assert_eq!(b.hidden_next, followers);
        }
        let names: Vec<&str> = habitat.next.iter().map(|b| b.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "branches keep their places, by name");
    }

    /// DEV SEES EVERYTHING: an All view holds every quest of the chain and every step, each
    /// marked with how far the player has got, so the page can dim what is not reached.
    #[test]
    fn an_all_view_holds_every_quest_and_step_marked() {
        let (reg, tracker) = mid_chain();
        let view = plain_words(|w| build(&reg, &tracker, "b", Reveal::All, w)).expect("b is in the registry");
        let ids: Vec<&str> = view.quests().iter().map(|q| q.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c", "d", "e"]);
        for q in view.quests() {
            assert_eq!(q.steps.len(), q.step_total, "{} shows every step", q.id);
            assert_eq!(q.hidden_next, 0);
        }
        let b = &view.quests()[1];
        assert_eq!(b.steps.iter().map(|s| s.state).collect::<Vec<_>>(), [StepState::Done, StepState::Current, StepState::Ahead]);
        let states: Vec<QuestState> = view.quests().iter().map(|q| q.state).collect();
        assert_eq!(states, [QuestState::Done, QuestState::Active, QuestState::Locked, QuestState::Locked, QuestState::Locked]);
        assert!(view.quests()[2].steps.iter().all(|s| s.state == StepState::Ahead));
    }

    /// A quest the player has not reached has no Learned chain at all, and the top of a chain is
    /// found through any loop in the data without hanging.
    #[test]
    fn a_locked_focus_has_no_learned_view_and_loops_end() {
        let (reg, tracker) = mid_chain();
        assert!(plain_words(|w| build(&reg, &tracker, "e", Reveal::Learned, w)).is_none());
        assert!(plain_words(|w| build(&reg, &tracker, "e", Reveal::All, w)).is_some());
        let mut looped = QuestRegistry::default();
        for q in [quest("x", 1, Some("y")), quest("y", 1, Some("x"))] {
            looped.quests.insert(q.id.clone(), q);
        }
        assert!(top_of(&looped, "x").is_some());
        let view = plain_words(|w| build(&looped, &QuestTracker::default(), "x", Reveal::All, w)).expect("in the registry");
        assert_eq!(view.quests().len(), 2, "each quest once");
        assert_eq!(top_of(&reg, "e").map(|(t, d)| (t.id.as_str(), d)), Some(("a", 3)));
    }
}

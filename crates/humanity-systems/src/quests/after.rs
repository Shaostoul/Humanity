//! What must be finished before a quest starts: its `after` (2026-11-20, engine piece P1 of
//! docs/design/quest-chains.md, section 1.4, "Joins").
//!
//! A quest used to name one `prerequisite`, so a chain was a tree and a quest could follow only
//! one other. Some quests make sense after ANY of several: Start Farming follows Build First
//! Habitat or Grow Your Own, whichever the player did; "Water for Bad Days" follows whichever way
//! of making water safe they chose. `after` says that, and replaces `prerequisite` outright (no
//! compatibility shim before v1.0, CLAUDE.md):
//!
//! ```ron
//! after: None                                   // offered: shown under Available
//! after: Quest("gs_first_steps")                // after that one
//! after: AnyOf(["tutorial_first_habitat", "food_grow"])   // after whichever comes first
//! after: AllOf(["water_first", "food_first"])   // after every one (capstones only)
//! ```
//!
//! When a quest completes, every quest whose `after` names it and is now met, and that is neither
//! active nor done, starts (`QuestSystem::tick`). A quest whose `after` was met before it existed
//! (one added in an update) is offered under Available instead.

use serde::{Deserialize, Serialize};

/// What must be finished before a quest starts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum After {
    /// Nothing: the quest is offered (or is a start's opening).
    #[default]
    None,
    /// That one quest.
    Quest(String),
    /// Any one of these (a join: whichever branch the player took).
    AnyOf(Vec<String>),
    /// Every one of these.
    AllOf(Vec<String>),
}

impl After {
    /// The quests it names, in the order written.
    pub fn ids(&self) -> &[String] {
        match self {
            After::None => &[],
            After::Quest(q) => std::slice::from_ref(q),
            After::AnyOf(v) | After::AllOf(v) => v,
        }
    }

    /// Whether it names `id`.
    pub fn names(&self, id: &str) -> bool {
        self.ids().iter().any(|q| q == id)
    }

    /// Whether it is met, `done` saying whether a quest is finished. `None` is always met; an
    /// empty `AnyOf` never is (the shipped-quest test refuses one).
    pub fn met(&self, done: impl Fn(&str) -> bool) -> bool {
        match self {
            After::None => true,
            After::Quest(q) => done(q),
            After::AnyOf(v) => v.iter().any(|q| done(q)),
            After::AllOf(v) => v.iter().all(|q| done(q)),
        }
    }

    /// Whether it names nothing.
    pub fn is_none(&self) -> bool {
        matches!(self, After::None)
    }

    /// The quest drawn above this one in a chain window: the first it names that `done` says is
    /// finished (the branch the player took), else the first it names.
    pub fn parent(&self, done: impl Fn(&str) -> bool) -> Option<&str> {
        let ids = self.ids();
        ids.iter().find(|q| done(q)).or_else(|| ids.first()).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_form_reads_from_ron_and_says_when_it_is_met() {
        let read = |s: &str| ron::from_str::<After>(s).unwrap_or_else(|e| panic!("{s}: {e}"));
        assert_eq!(read("None"), After::None);
        assert_eq!(read(r#"Quest("a")"#), After::Quest("a".into()));
        let any = read(r#"AnyOf(["a", "b"])"#);
        let all = read(r#"AllOf(["a", "b"])"#);
        let only_b = |q: &str| q == "b";
        assert!(After::None.met(only_b));
        assert!(!After::Quest("a".into()).met(only_b));
        assert!(any.met(only_b), "AnyOf is met by any one of its quests");
        assert!(!all.met(only_b), "AllOf waits for every one");
        assert!(all.met(|_| true));
        assert!(!After::AnyOf(vec![]).met(|_| true), "an empty AnyOf is never met");
        assert_eq!(any.parent(only_b), Some("b"), "the branch taken is drawn above");
        assert_eq!(any.parent(|_| false), Some("a"));
        assert!(any.names("a") && !any.names("c"));
    }
}

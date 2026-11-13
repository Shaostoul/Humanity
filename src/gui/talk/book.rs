//! The talk card's written choices (data/npc/talk.ron) and who each crew member is (data/npc/crew.ron),
//! as the client reads them (rung 3 of docs/design/embeddable-ai.md, 2026-11-12).
//!
//! The relay owns the crew in the world (src/relay/handlers/game_state.rs reads crew.ron to spawn them
//! and sends each one's name, chore line, dialog and greetings); the card's TOPICS, the tasks a crew
//! member offers, whether they trade, the places they can show the way to, and the persona an AI model
//! is given are read here from the same data folder, by the crew member's name and role. A crew member
//! this client's data does not know (a server with its own crew) still gets a card: the relay's lines,
//! `everyone`'s topics and places, and no persona (so no AI box), never an error.
//!
//! Read from disk each time a card opens (a few KB): edit talk.ron while the game runs and the next
//! card shows it. Nothing is kept between opens (CLAUDE.md's no-caches rule).

use std::collections::BTreeMap;

use serde::Deserialize;

const BUILTIN_TALK: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/npc/talk.ron"));
const BUILTIN_CREW: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/npc/crew.ron"));

/// One level of talk.ron (`everyone`, a role, or a person). Every field is optional so a level names
/// only what it changes.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TalkDef {
    /// Lines by topic id.
    pub lines: BTreeMap<String, Vec<String>>,
    /// Tasks offered: quest ids.
    pub work: Option<Vec<String>>,
    pub trades: Option<bool>,
    /// Places they can show the way to: destination ids.
    pub ways: Option<Vec<String>>,
    /// What they say when they show the way (`{place}`).
    pub way_lines: Option<Vec<String>>,
    /// What they say when a task is taken on (`{task}`).
    pub accept_lines: Option<Vec<String>>,
}

/// data/npc/talk.ron.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TalkBook {
    pub topics: Vec<String>,
    pub everyone: TalkDef,
    #[serde(default)]
    pub roles: BTreeMap<String, TalkDef>,
    #[serde(default)]
    pub people: BTreeMap<String, TalkDef>,
}

/// What one crew member's card offers, resolved person over role over everyone.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Resolved {
    /// (topic id, its lines) in the book's topic order; a topic with no line is left out.
    pub topics: Vec<(String, Vec<String>)>,
    pub work: Vec<String>,
    pub trades: bool,
    pub ways: Vec<String>,
    pub way_lines: Vec<String>,
    pub accept_lines: Vec<String>,
}

impl TalkBook {
    pub fn parse(text: &str) -> Result<TalkBook, String> {
        humanity_core::data_parse::parse_ron::<TalkBook>(text.as_bytes())
    }

    /// The data folder's talk.ron, else the copy built into the app (logged).
    pub fn current() -> TalkBook {
        let path = humanity_core::storage::data_dir().join("npc").join("talk.ron");
        match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| TalkBook::parse(&t)) {
            Ok(b) => b,
            Err(e) => {
                humanity_core::embedded_data::note_builtin_copy("npc/talk.ron", format_args!("{} could not be used ({e})", path.display()));
                TalkBook::parse(BUILTIN_TALK).expect("the built-in talk.ron parses")
            }
        }
    }

    /// The card of the crew member called `name` whose role is `role` (an unknown role or name falls
    /// back to `everyone`).
    pub fn resolve(&self, name: &str, role: &str) -> Resolved {
        let levels: Vec<&TalkDef> = [Some(&self.everyone), self.roles.get(role), self.people.get(name)].into_iter().flatten().collect();
        let most_specific = |pick: &dyn Fn(&TalkDef) -> Option<Vec<String>>| levels.iter().rev().find_map(|d| pick(d)).unwrap_or_default();
        let topics = self
            .topics
            .iter()
            .filter_map(|t| levels.iter().rev().find_map(|d| d.lines.get(t).filter(|l| !l.is_empty())).map(|l| (t.clone(), l.clone())))
            .collect();
        Resolved {
            topics,
            work: most_specific(&|d| d.work.clone()),
            trades: levels.iter().rev().find_map(|d| d.trades).unwrap_or(false),
            ways: most_specific(&|d| d.ways.clone()),
            way_lines: most_specific(&|d| d.way_lines.clone()),
            accept_lines: most_specific(&|d| d.accept_lines.clone()),
        }
    }
}

/// One crew member as data/npc/crew.ron writes them (the fields the card and the persona need; the
/// relay's own reader, game_state.rs `CrewDef`, takes the rest).
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct CrewRow {
    pub post: String,
    pub name: String,
    pub role: String,
    pub description: String,
    /// Who they are and how they talk, for an AI model (8.2): written by people.
    #[serde(default)]
    pub persona: String,
    #[serde(default)]
    pub dialog: Vec<String>,
    #[serde(default)]
    pub greetings: Vec<String>,
}

/// data/npc/crew.ron, the data folder's copy else the built-in one.
pub fn crew() -> Vec<CrewRow> {
    let path = humanity_core::storage::data_dir().join("npc").join("crew.ron");
    let parse = |t: &str| humanity_core::data_parse::parse_ron::<Vec<CrewRow>>(t.as_bytes());
    match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| parse(&t)) {
        Ok(c) => c,
        Err(e) => {
            humanity_core::embedded_data::note_builtin_copy("npc/crew.ron", format_args!("{} could not be used ({e})", path.display()));
            parse(BUILTIN_CREW).expect("the built-in crew.ron parses")
        }
    }
}

/// A role id in words for the persona and the card ("medical_officer" becomes "medical officer").
pub fn role_words(role: &str) -> String {
    role.replace(['_', '-'], " ")
}

/// A post (a room id) in words ("mess-hall" becomes "the mess hall", "commons" "the Commons").
pub fn post_words(post: &str) -> String {
    match post {
        "commons" => "the Commons".into(),
        other => format!("the {}", other.replace(['_', '-'], " ")),
    }
}

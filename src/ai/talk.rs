//! Talking characters, the AI half (docs/design/embeddable-ai.md, 8.2, 8.3 and section 14 rung 3;
//! built 2026-11-12). Pure functions, no engine and no GUI: the talk card (src/gui/talk/) drives the
//! engine through the app's one client and hands what comes back through here.
//!
//! - [`persona`]: the project's own prompt (data/ai/prompts/character.txt, versioned, never a model
//!   repository's default) filled from the character's row of data/npc/crew.ron and the live facts the
//!   client has. The part every character shares comes first and the live facts last, so the engine's
//!   prompt reuse (9.6) reads the stable start once per conversation.
//! - [`grammar`]: THE FIXED OUTPUT SHAPE (8.3, llama.cpp's grammar-constrained sampling). A reply is
//!   `{"action":"...","line":"..."}` and nothing else: the line is words and plain punctuation
//!   ([`LINE_CHARS`]), at most [`MAX_LINE_CHARS`] long; the action, chosen FIRST (with the line first
//!   Ministral wrote its action's name as its words, 2026-11-12), is `none` or one
//!   of the actions THIS character's card grants (offer one of its tasks, open trade if it trades, point
//!   the way to one of its places). The grammar is written per character, so the model cannot even
//!   produce an action the game did not grant.
//! - [`parse_reply`]: the shape checked again on the app's side, an action the game did not grant
//!   refused (the grammar is the first guard, this the second: an engine bug, or a different engine,
//!   never gets an ungranted action through).
//! - [`Filter`]: the deterministic filter of 8.3 (data/ai/filter.ron), run on the line AS IT STREAMS.
//! - [`streaming_line`]: the line so far, out of the JSON as it arrives, shown word by word.
//!
//! NOTHING HERE ACTS. An action is a suggestion the card shows as a button the player clicks (8.5: "AI
//! suggests; the person clicks"); a line is words on the player's own screen, never sent anywhere
//! (8.1), tagged AI wherever it is shown (5.4).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Deserialize;

/// The longest line a character may say, characters: the grammar's cap, and the filter's
/// (data/ai/filter.ron says the same; a test holds the two together).
pub const MAX_LINE_CHARS: usize = 240;

/// Tokens a reply may take: the JSON around the line (about 15), a line of up to three short sentences
/// (40 to 80), and an action (up to 10). The grammar closes the JSON before the line reaches
/// [`MAX_LINE_CHARS`], so a reply ends inside this.
pub const MAX_TOKENS: u32 = 160;

/// How many turns of this conversation go back to the model with each question (the player's and the
/// character's, oldest dropped first): enough to keep a thread, small enough that the prompt stays
/// quick to read. Memory only, gone when the card closes (8.1; question 6 is whether characters
/// remember past conversations, not built).
pub const HISTORY_TURNS: usize = 6;

/// The project's prompt as the app ships it (data/ai/prompts/character.txt), the copy used when the
/// data folder's cannot be read.
const BUILTIN_PROMPT: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/ai/prompts/character.txt"));
const BUILTIN_FILTER: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/ai/filter.ron"));

/// An action a character's reply may offer, which the player then clicks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Offer one of the tasks the card lists under Work (a quest id).
    OfferTask(String),
    /// Offer to trade (only a character who trades).
    OpenTrade,
    /// Show the way to one of the card's places (a destination id).
    PointWay(String),
}

impl Action {
    /// How the action is written in a reply and in the grammar: `offer_task:<id>`, `open_trade`,
    /// `point_way:<id>`.
    pub fn wire(&self) -> String {
        match self {
            Action::OfferTask(id) => format!("offer_task:{id}"),
            Action::OpenTrade => "open_trade".into(),
            Action::PointWay(id) => format!("point_way:{id}"),
        }
    }

    /// The action a reply names, if it is one of the three kinds (whether it was GRANTED is
    /// [`Grants::allows`]).
    pub fn from_wire(s: &str) -> Option<Action> {
        if s == "open_trade" {
            return Some(Action::OpenTrade);
        }
        let ok = |id: &str| !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if let Some(id) = s.strip_prefix("offer_task:").filter(|id| ok(id)) {
            return Some(Action::OfferTask(id.into()));
        }
        s.strip_prefix("point_way:").filter(|id| ok(id)).map(|id| Action::PointWay(id.into()))
    }
}

/// What THIS character's card grants: the only actions its replies may offer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Grants {
    /// Tasks it offers now: (quest id, the task's name).
    pub tasks: Vec<(String, String)>,
    /// Whether it trades.
    pub trade: bool,
    /// Places it can show the way to: (destination id, the place's name).
    pub ways: Vec<(String, String)>,
}

impl Grants {
    /// Every granted action, in the card's order: tasks, trade, the ways.
    pub fn actions(&self) -> Vec<Action> {
        let mut out: Vec<Action> = self.tasks.iter().map(|(id, _)| Action::OfferTask(id.clone())).collect();
        if self.trade {
            out.push(Action::OpenTrade);
        }
        out.extend(self.ways.iter().map(|(id, _)| Action::PointWay(id.clone())));
        out
    }

    pub fn allows(&self, a: &Action) -> bool {
        match a {
            Action::OfferTask(id) => self.tasks.iter().any(|(t, _)| t == id),
            Action::OpenTrade => self.trade,
            Action::PointWay(id) => self.ways.iter().any(|(w, _)| w == id),
        }
    }

    /// The prompt's list of actions: one line each, the action as written and what it means.
    fn prompt_lines(&self) -> String {
        let mut lines = Vec::new();
        for (id, name) in &self.tasks {
            lines.push(format!("- offer_task:{id} (offer the task \"{name}\")"));
        }
        if self.trade {
            lines.push("- open_trade (offer to trade)".into());
        }
        for (id, name) in &self.ways {
            lines.push(format!("- point_way:{id} (show the way to {name})"));
        }
        if lines.is_empty() {
            lines.push("- none (you offer nothing else here)".into());
        }
        lines.join("\n")
    }
}

/// Who the character is, for the persona: crew.ron's row and the live facts the client has.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Character {
    pub name: String,
    /// The job, in words ("navigator"; crew.ron's role made readable).
    pub job: String,
    /// crew.ron's `description` ("Off the helm, keeping the ship's notices").
    pub description: String,
    /// The post, in words ("the Commons").
    pub post: String,
    /// crew.ron's `persona`: a few sentences of who they are and how they talk, written by people.
    pub persona: String,
    /// Their written lines, as examples of their voice (crew.ron's dialog and greetings).
    pub examples: Vec<String>,
    /// What they are doing right now (the live chore line), the one live fact.
    pub doing: String,
}

/// The project's prompt: its version and its text (the `#` notes taken out).
#[derive(Debug, Clone, PartialEq)]
pub struct Prompt {
    pub version: u32,
    pub body: String,
}

impl Prompt {
    /// Read a prompt file: a `version: N` line first, `#` lines are notes for people.
    pub fn parse(text: &str) -> Result<Prompt, String> {
        let mut lines = text.lines();
        let first = lines.next().unwrap_or("").trim();
        let version = first.strip_prefix("version:").and_then(|v| v.trim().parse::<u32>().ok()).ok_or_else(|| format!("the first line is {first:?}, not \"version: N\""))?;
        let body: Vec<&str> = lines.filter(|l| !l.trim_start().starts_with('#')).collect();
        let body = body.join("\n").trim().to_string();
        for needed in ["{actions}", "{name}", "{examples}", "{doing}"] {
            if !body.contains(needed) {
                return Err(format!("the prompt has no {needed}"));
            }
        }
        Ok(Prompt { version, body })
    }

    /// The character prompt: the data folder's copy (a mod may change it), else the one built in.
    pub fn character() -> Prompt {
        let path = humanity_core::storage::data_dir().join("ai").join("prompts").join("character.txt");
        match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| Prompt::parse(&t)) {
            Ok(p) => p,
            Err(e) => {
                humanity_core::embedded_data::note_builtin_copy("ai/prompts/character.txt", format_args!("{} could not be used ({e})", path.display()));
                Prompt::parse(BUILTIN_PROMPT).expect("the built-in character prompt parses")
            }
        }
    }
}

/// The whole persona (the system message) for one character.
pub fn persona(prompt: &Prompt, c: &Character, grants: &Grants) -> String {
    let examples: Vec<String> = c.examples.iter().take(6).map(|l| format!("- {l}")).collect();
    let persona = if c.persona.trim().is_empty() { String::new() } else { c.persona.trim().to_string() };
    let doing = if c.doing.trim().is_empty() { "at your post".to_string() } else { c.doing.trim().to_lowercase() };
    prompt
        .body
        .replace("{actions}", &grants.prompt_lines())
        .replace("{name}", &c.name)
        .replace("{job}", &c.job)
        .replace("{description}", &lower_first(c.description.trim_end_matches('.')))
        .replace("{post}", &c.post)
        .replace("{persona}", &persona)
        .replace("{examples}", &examples.join("\n"))
        .replace("{doing}", &doing)
}

/// What a line may be made of, as a GBNF character class: letters (English and the accented Latin
/// letters), digits, spaces and plain punctuation. No quote or backslash (the JSON stays exact), no
/// control character, no `_` (so an action's name such as point_way:mess_hall cannot be written into
/// the words; seen 2026-11-12, Ministral wrote its action as its line), no `#`, brackets or braces
/// (hashtags, template markers), and no emoji, which the app's font draws as boxes (seen 2026-11-12:
/// Granite ended a reply with hashtags and emoji).
pub const LINE_CHARS: &str = r"[A-Za-z0-9 .,!?'():;\u00C0-\u024F\u2019-]";

/// "Tending the Commons garden" as the middle of a sentence: "tending the Commons garden".
fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_lowercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// GBNF for one character's reply: the fixed shape with its granted actions (module docs).
pub fn grammar(grants: &Grants) -> String {
    let mut actions = vec!["\"none\"".to_string()];
    actions.extend(grants.actions().iter().map(|a| format!("\"{}\"", a.wire())));
    format!(
        "root ::= \"{{\\\"action\\\":\\\"\" action \"\\\",\\\"line\\\":\\\"\" line \"\\\"}}\"\nline ::= {LINE_CHARS}{{1,{MAX_LINE_CHARS}}}\naction ::= {}\n",
        actions.join(" | ")
    )
}

/// A reply in the fixed shape.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub line: String,
    pub action: Option<Action>,
}

/// Why a reply was refused (each has its words, `ai.talk.shape.*`).
#[derive(Debug, Clone, PartialEq)]
pub enum ShapeError {
    /// Not the JSON object of the shape (anything else: text, another object, more fields).
    NotTheShape(String),
    /// The line is empty or only spaces.
    Empty,
    /// It names an action that is not one of the kinds.
    UnknownAction(String),
    /// It names an action this character's card does not grant.
    NotGranted(String),
}

impl ShapeError {
    pub fn words_key(&self) -> &'static str {
        match self {
            ShapeError::NotTheShape(_) => "ai.talk.shape.not_the_shape",
            ShapeError::Empty => "ai.talk.shape.empty",
            ShapeError::UnknownAction(_) => "ai.talk.shape.unknown_action",
            ShapeError::NotGranted(_) => "ai.talk.shape.not_granted",
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    line: String,
    action: String,
}

/// Check a whole reply against the shape and the grants.
pub fn parse_reply(raw: &str, grants: &Grants) -> Result<Reply, ShapeError> {
    // A JSON object and nothing else: serde would also read the two fields out of an array.
    if !raw.trim_start().starts_with('{') {
        return Err(ShapeError::NotTheShape("not a JSON object".into()));
    }
    let w: Wire = serde_json::from_str(raw.trim()).map_err(|e| ShapeError::NotTheShape(e.to_string()))?;
    let line = w.line.trim().to_string();
    if line.is_empty() {
        return Err(ShapeError::Empty);
    }
    if line.chars().count() > MAX_LINE_CHARS || line.chars().any(|c| c.is_control()) {
        return Err(ShapeError::NotTheShape(format!("a line of {} characters or with control characters", line.chars().count())));
    }
    let action = match w.action.as_str() {
        "none" => None,
        other => {
            let a = Action::from_wire(other).ok_or_else(|| ShapeError::UnknownAction(other.to_string()))?;
            if !grants.allows(&a) {
                return Err(ShapeError::NotGranted(other.to_string()));
            }
            Some(a)
        }
    };
    Ok(Reply { line, action })
}

/// The line so far, out of a reply still arriving: what follows `"line":"` up to its closing quote
/// (or the end so far). Empty before the line starts, and for anything not in the shape.
pub fn streaming_line(partial: &str) -> &str {
    const OPEN: &str = "\"line\":\"";
    let p = partial.trim_start();
    if !p.starts_with("{\"action\":\"") {
        return "";
    }
    let Some(at) = p.find(OPEN) else { return "" };
    let rest = &p[at + OPEN.len()..];
    match rest.find('"') {
        Some(end) => &rest[..end],
        None => rest,
    }
}

/// One rule of the filter.
#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub why: String,
    pub phrases: Vec<String>,
}

/// The filter of 8.3 (data/ai/filter.ron, written as `TalkFilter(...)`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename = "TalkFilter")]
pub struct Filter {
    pub max_chars: usize,
    pub markers: Vec<String>,
    pub rules: Vec<Rule>,
}

/// A line the filter stopped: the rule's name (its words are `ai.talk.filter.<why>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterHit {
    pub why: String,
}

/// Lowercase words separated by single spaces, with a space at each end, so a phrase matches only as
/// whole words.
fn words_of(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push(' ');
    let mut gap = false;
    for c in s.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            if gap && out.len() > 1 {
                out.push(' ');
            }
            gap = false;
            out.push(c);
        } else {
            gap = true;
        }
    }
    out.push(' ');
    out
}

impl Filter {
    pub fn parse(text: &str) -> Result<Filter, String> {
        humanity_core::data_parse::parse_ron::<Filter>(text.as_bytes())
    }

    /// The data folder's filter, else the one built in.
    pub fn current() -> Filter {
        let path = humanity_core::storage::data_dir().join("ai").join("filter.ron");
        match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| Filter::parse(&t)) {
            Ok(f) => f,
            Err(e) => {
                humanity_core::embedded_data::note_builtin_copy("ai/filter.ron", format_args!("{} could not be used ({e})", path.display()));
                Filter::parse(BUILTIN_FILTER).expect("the built-in filter parses")
            }
        }
    }

    /// Check a line (whole or still arriving): too long, a template marker, or a rule's phrase.
    pub fn check(&self, line: &str) -> Result<(), FilterHit> {
        if line.chars().count() > self.max_chars {
            return Err(FilterHit { why: "too_long".into() });
        }
        if self.markers.iter().any(|m| line.contains(m.as_str())) {
            return Err(FilterHit { why: "marker".into() });
        }
        let words = words_of(line);
        for rule in &self.rules {
            if rule.phrases.iter().any(|p| words.contains(&words_of(p))) {
                return Err(FilterHit { why: rule.why.clone() });
            }
        }
        Ok(())
    }
}

/// This session's counts for the Dev page (12.1: "filter replacements, fallbacks"): memory only.
#[derive(Debug, Default)]
pub struct TalkCounts {
    pub asked: AtomicU64,
    pub answered: AtomicU64,
    pub filtered: AtomicU64,
    pub refused_shape: AtomicU64,
    pub fell_back: AtomicU64,
    pub timed_out: AtomicU64,
}

pub static COUNTS: TalkCounts = TalkCounts {
    asked: AtomicU64::new(0),
    answered: AtomicU64::new(0),
    filtered: AtomicU64::new(0),
    refused_shape: AtomicU64::new(0),
    fell_back: AtomicU64::new(0),
    timed_out: AtomicU64::new(0),
};

impl TalkCounts {
    pub fn bump(c: &AtomicU64) {
        c.fetch_add(1, Ordering::Relaxed);
    }

    /// (asked, answered, filtered, refused for their shape, fell back, timed out).
    pub fn read(&self) -> [u64; 6] {
        [&self.asked, &self.answered, &self.filtered, &self.refused_shape, &self.fell_back, &self.timed_out].map(|c| c.load(Ordering::Relaxed))
    }
}

/// The filter rules by name, for the words test (every `why` has its words).
pub fn filter_reasons(f: &Filter) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for why in ["too_long", "marker"] {
        m.insert(why.to_string(), 0);
    }
    for r in &f.rules {
        m.insert(r.why.clone(), r.phrases.len());
    }
    m
}

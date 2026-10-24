//! The how-to data model: one procedure as data (docs/design/how-to.md, "The model"; every field
//! is described in schemas/how_to.toml, which scripts/build-how-to.js reads as the shape check).
//!
//! Every struct refuses fields it does not know (`deny_unknown_fields`): a typo such as `shrot`
//! for `short` would otherwise be dropped in silence, and that one is the Short view's safety gate.
//! tests/fixtures/how_to/base.json uses every field schemas/how_to.toml lists, and parsing it here
//! proves this model knows every one of them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One how-to, as its file holds it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HowTo {
    pub id: String,
    pub title: String,
    pub summary: String,
    /// Who it is for ("for" in the file; a Rust keyword).
    #[serde(rename = "for")]
    pub for_whom: String,
    #[serde(default)]
    pub audience: Audience,
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub realities: Vec<Reality>,
    /// Which HumanityOS apps the steps are written for; empty means both, or that it does not matter.
    #[serde(default)]
    pub apps: Vec<App>,
    pub time: Time,
    pub difficulty: u8,
    pub hazard: Hazard,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub reading: Vec<String>,
    #[serde(default)]
    pub needs_first: Vec<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub tools: Vec<Need>,
    #[serde(default)]
    pub materials: Vec<Need>,
    #[serde(default)]
    pub stop_and_call: Vec<StopAndCall>,
    pub steps: Vec<Step>,
    #[serde(default)]
    pub worked_if: Vec<String>,
    #[serde(default)]
    pub problems: Vec<Problem>,
    #[serde(default)]
    pub help: Option<Help>,
    #[serde(default)]
    pub repeat: Option<Repeat>,
    #[serde(default)]
    pub makes: Option<Makes>,
    pub sources: Vec<Source>,
    pub review: Review,
    pub written_by: WrittenBy,
    pub updated: String,
}

/// Who sees a how-to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Audience {
    #[default]
    Everyone,
    ServerOwner,
    Developer,
}

/// Real life, the game, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reality {
    Real,
    Sim,
}

/// A HumanityOS app the steps are written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum App {
    Desktop,
    Website,
}

/// How long it takes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Time {
    pub active_min: u32,
    pub waiting_min: u32,
}

/// How badly a person can be hurt by doing it wrong: the Curriculum's scale. Ordered, so the
/// how-to's hazard can be compared with its worst step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Hazard {
    None,
    Caution,
    Serious,
    Lethal,
}

impl Hazard {
    /// The id the data and the words file use.
    pub fn id(self) -> &'static str {
        match self {
            Hazard::None => "none",
            Hazard::Caution => "caution",
            Hazard::Serious => "serious",
            Hazard::Lethal => "lethal",
        }
    }
}

/// A step's safety level: the hazard scale without `none`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Caution,
    Serious,
    Lethal,
}

impl Level {
    /// The same level on the how-to's scale.
    pub fn hazard(self) -> Hazard {
        match self {
            Level::Caution => Hazard::Caution,
            Level::Serious => Hazard::Serious,
            Level::Lethal => Hazard::Lethal,
        }
    }

    /// Serious or lethal: the levels Short may never drop and whose numbers must cite.
    pub fn is_grave(self) -> bool {
        matches!(self, Level::Serious | Level::Lethal)
    }
}

/// A tool or a material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Need {
    #[serde(default)]
    pub item: Option<String>,
    #[serde(default)]
    pub proposed_item: Option<String>,
    #[serde(default)]
    pub qty: Option<Qty>,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub note: Option<String>,
}

/// An amount, whole, in a metric unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qty {
    #[serde(default)]
    pub value: Option<u64>,
    #[serde(default)]
    pub min: Option<u64>,
    #[serde(default)]
    pub max: Option<u64>,
    pub unit: Unit,
}

/// The units a quantity may be stored in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Each,
    G,
    Kg,
    Ml,
    L,
    Mm,
    Cm,
    M,
}

impl Unit {
    pub fn id(self) -> &'static str {
        match self {
            Unit::Each => "each",
            Unit::G => "g",
            Unit::Kg => "kg",
            Unit::Ml => "ml",
            Unit::L => "l",
            Unit::Mm => "mm",
            Unit::Cm => "cm",
            Unit::M => "m",
        }
    }
}

/// When to stop and get help instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StopAndCall {
    pub when: String,
    #[serde(rename = "do")]
    pub then: String,
    #[serde(default)]
    pub who: Option<String>,
    #[serde(default)]
    pub cites: Vec<String>,
}

/// What a step asks of the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Do,
    Wait,
    Check,
    Measure,
    Choose,
    Call,
    Go,
    Action,
    Command,
}

impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Kind::Do => "do",
            Kind::Wait => "wait",
            Kind::Check => "check",
            Kind::Measure => "measure",
            Kind::Choose => "choose",
            Kind::Call => "call",
            Kind::Go => "go",
            Kind::Action => "action",
            Kind::Command => "command",
        }
    }
}

/// One step. The kind-specific fields sit on the step itself (flat, as the file reads); the
/// checker's `kind_fields` gate holds each kind to its own fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub id: String,
    pub kind: Kind,
    pub text: String,
    #[serde(default)]
    pub why: Vec<String>,
    #[serde(default)]
    pub detail: Vec<String>,
    #[serde(default)]
    pub safety: Option<Safety>,
    #[serde(default)]
    pub uses: Vec<String>,
    #[serde(default)]
    pub image: Option<Image>,
    #[serde(default)]
    pub cites: Vec<String>,
    #[serde(default)]
    pub time_min: Option<u32>,
    #[serde(default)]
    pub who: Option<String>,
    /// `if` in the file (a Rust keyword): choose step id (or `place`) to the option it needs.
    #[serde(default, rename = "if")]
    pub when: BTreeMap<String, String>,
    #[serde(default)]
    pub sim: Option<Sim>,
    /// `false` leaves an explanatory step out of the Short view.
    #[serde(default)]
    pub short: Option<bool>,
    #[serde(default)]
    pub no: Option<String>,
    #[serde(default)]
    pub wait: Option<Wait>,
    #[serde(default)]
    pub record: Option<Record>,
    #[serde(default)]
    pub options: Vec<Choice>,
    #[serde(default)]
    pub how_to: Option<String>,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub section: Option<String>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub shell: Option<Shell>,
    #[serde(default)]
    pub ai_notes: Vec<String>,
}

impl Step {
    /// The step's safety level, when it has a note.
    pub fn level(&self) -> Option<Level> {
        self.safety.as_ref().map(|s| s.level)
    }

    /// Whether the Short view shows it. A serious or lethal step always shows: Short shortens, it
    /// never omits (the checker also refuses `short: false` on one, gate short_hides_safety, so this
    /// is the second of two locks, not the only one).
    pub fn in_short(&self) -> bool {
        self.short != Some(false) || self.level().map(|l| l.is_grave()).unwrap_or(false)
    }
}

/// A safety note at a step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Safety {
    pub level: Level,
    #[serde(default)]
    pub short: Option<String>,
    #[serde(default)]
    pub full: Vec<String>,
}

/// A picture at a step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub file: String,
    #[serde(default)]
    pub alt: Option<String>,
}

/// How the game sees a step (increment 5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sim {
    pub mode: SimMode,
    #[serde(default)]
    pub kind: Option<SimKind>,
    #[serde(default)]
    pub target: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimMode {
    Watch,
    Read,
    Skip,
}

/// The quest objective kinds (src/systems/quests/objectives.rs) that can tick a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimKind {
    Gather,
    Craft,
    Make,
    Harvest,
    Build,
    Travel,
    Talk,
    Eat,
    Plant,
    View,
}

/// A wait step's timer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wait {
    pub min_s: u64,
    #[serde(default)]
    pub max_s: Option<u64>,
}

/// A measure step's value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    #[serde(rename = "type")]
    pub kind: RecordKind,
    pub label: String,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub range: Option<Range>,
    #[serde(default)]
    pub remember: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordKind {
    Text,
    Number,
}

/// The normal values of a number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Range {
    #[serde(default)]
    pub min: Option<u64>,
    #[serde(default)]
    pub max: Option<u64>,
}

/// One option of a choose step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub id: String,
    pub label: String,
}

/// Where a command step is typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shell {
    Bash,
    Powershell,
    Cmd,
}

/// What goes wrong.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Problem {
    pub id: String,
    pub sign: String,
    #[serde(default)]
    pub why: Option<String>,
    pub fix: String,
    #[serde(default)]
    pub stop: bool,
    #[serde(default)]
    pub cites: Vec<String>,
}

/// Who could do it for you, or teach it (increment 2 renders it).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Help {
    #[serde(default, rename = "do")]
    pub verbs: Vec<String>,
    #[serde(default)]
    pub acts_on: Vec<String>,
    #[serde(default)]
    pub teach: bool,
    #[serde(default)]
    pub catalog: Vec<String>,
}

/// The suggested interval for doing it again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Repeat {
    pub every_days: u32,
    #[serde(default)]
    pub note: Option<String>,
}

/// What it makes in the game: a recipe id now, the transform itself from increment 4.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Makes {
    pub recipe: String,
    #[serde(default)]
    pub inputs: Vec<Count>,
    #[serde(default)]
    pub outputs: Vec<Count>,
    #[serde(default)]
    pub seconds: Option<u64>,
    #[serde(default)]
    pub station: Option<String>,
    #[serde(default)]
    pub skill: Option<String>,
    #[serde(default)]
    pub skill_level: Option<u32>,
}

/// An item and a count, in a recipe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Count {
    pub item: String,
    pub count: u64,
}

/// Where a fact comes from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub key: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub in_project: Option<String>,
    pub title: String,
    #[serde(default, rename = "where")]
    pub at: Option<String>,
    pub dated: String,
    pub read: String,
    #[serde(default)]
    pub quote: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

/// The review: a field, not a comment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub status: Status,
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub checked: Option<String>,
    #[serde(default)]
    pub by: Option<String>,
    #[serde(default)]
    pub person: Option<String>,
}

/// The Curriculum's review scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Absent,
    Stub,
    Sourced,
    Verified,
}

impl Status {
    pub fn id(self) -> &'static str {
        match self {
            Status::Absent => "absent",
            Status::Stub => "stub",
            Status::Sourced => "sourced",
            Status::Verified => "verified",
        }
    }
}

/// Who wrote it, shown on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WrittenBy {
    Ai,
    Person,
}

/// One node of data/how_to/categories.json.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Category {
    pub id: String,
    pub title: String,
    pub line: String,
    #[serde(default)]
    pub order: u32,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub subjects: Vec<String>,
    #[serde(default)]
    pub help: Vec<String>,
    #[serde(default)]
    pub audience: Audience,
    #[serde(default)]
    pub children: Vec<Category>,
}

/// data/how_to/categories.json.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CategoriesFile {
    #[serde(default)]
    pub categories: Vec<Category>,
}

/// A how-to named but not written yet (data/how_to/planned.json).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Planned {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub category: String,
    #[serde(default)]
    pub audience: Audience,
}

/// data/how_to/planned.json.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PlannedFile {
    #[serde(default)]
    pub planned: Vec<Planned>,
}

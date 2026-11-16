//! Practice quality (skills rung 2, 2026-11-15): docs/design/skills.md section 3, "What counts as
//! practice", points 3 and 4, approved by the operator on 2026-11-14.
//!
//! - **Variety.** The same task repeated loses value over an in-game day and recovers after. A task
//!   is one verb on one thing (`SkillXPEvent::task`: collecting one item, building one blueprint,
//!   crafting one recipe, harvesting one crop), so the same verb on another material or another
//!   recipe is a different task. The first attempts in a rest window count in full, each after that
//!   is worth less, down to a floor, and each attempt stops counting a day after it was made. The
//!   very first attempt at a task never done before is worth more.
//! - **Help.** Reading, to its end, the Library guide written for the stage you are at
//!   (`data/skills/guides.json`) multiplies that skill's practice for some in-game hours while you
//!   stay at that stage, once per guide per stage; practising within reach of someone two stages
//!   above you in the skill (a crew member with the skill, `data/npc/crew.ron`'s `skills`)
//!   multiplies the attempt, and the screen says so while it applies.
//!
//! Every number is data, `data/skills/practice_quality.json`, one block per level of the
//! `skill_learning` setting (data/gameplay/levels.json: Off, Simplified, Realistic). Off leaves
//! every attempt as it was; Simplified (a new player's) is the gentle curve; Realistic the full one.
//!
//! The memory of what was practised when (`PracticeMemory`) is part of the player (`PlayerSkills`)
//! and of the save, so quitting does not reset a day's repeats; the explanation of the last attempt
//! (`PracticeWhy`) is for the Skills page and is not saved. No words live here: the engine turns
//! these facts into keys of data/i18n/en.json (src/engine/practice_view.rs).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use humanity_core::hot_reload::data_store::DataStore;
use serde::{Deserialize, Serialize};

/// The practice-quality table under data/.
pub const FILE: &str = "skills/practice_quality.json";
/// The guide-per-stage table under data/.
pub const GUIDES_FILE: &str = "skills/guides.json";
/// DataStore: the loaded [`PracticeQuality`]. Absent (most unit tests): practice is weighed as in
/// rung 1, with no variety or help.
pub const DATA_KEY: &str = "practice_quality";
/// DataStore: `Mutex<LearningMode>`, the chosen level of Settings > Gameplay > Skill learning,
/// published each frame. Absent: the default level.
pub const MODE_KEY: &str = "skill_learning";
/// DataStore: `Mutex<Vec<Helper>>`, the people near the player who could teach, refreshed each
/// frame by the engine.
pub const HELPERS_KEY: &str = "practice_helpers";
/// DataStore: `Mutex<Vec<String>>`, Library guides read to their end this frame (their slugs).
pub const GUIDE_READS_KEY: &str = "guide_reads";
/// DataStore: `Mutex<Vec<GuideNotice>>`, what reading a guide did, for the screen to say.
pub const NOTICES_KEY: &str = "practice_notices";

/// The `skill_learning` setting's levels (data/gameplay/levels.json). Rung 4 adds what each does
/// to stages and assessment (skills.md section 3, "The two modes"); this rung reads it for practice
/// quality only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LearningMode {
    /// Every attempt counts the same: no repeats penalty, no guide or teacher bonus.
    Off,
    /// The gentle curve (a new player's).
    #[default]
    Simplified,
    /// The full curve.
    Realistic,
}

impl LearningMode {
    /// The level id levels.json and config.json use, or None for an id this system does not know.
    pub fn from_level(level: &str) -> Option<Self> {
        match level {
            "off" => Some(Self::Off),
            "simplified" => Some(Self::Simplified),
            "realistic" => Some(Self::Realistic),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Simplified => "simplified",
            Self::Realistic => "realistic",
        }
    }
}

/// Register the channels this module reads and writes (the main loop and the headless sim).
pub fn register(data: &mut DataStore) {
    data.insert(MODE_KEY, std::sync::Mutex::new(LearningMode::default()));
    data.insert(HELPERS_KEY, std::sync::Mutex::new(Vec::<Helper>::new()));
    data.insert(GUIDE_READS_KEY, std::sync::Mutex::new(Vec::<String>::new()));
    data.insert(NOTICES_KEY, std::sync::Mutex::new(Vec::<GuideNotice>::new()));
}

/// The chosen level into its channel (each frame).
pub fn publish_mode(data: &DataStore, mode: LearningMode) {
    if let Some(Ok(mut m)) = data.get::<std::sync::Mutex<LearningMode>>(MODE_KEY).map(|m| m.lock()) {
        *m = mode;
    }
}

/// The level in force: the channel's, else the default.
pub fn mode_of(data: &DataStore) -> LearningMode {
    data.get::<std::sync::Mutex<LearningMode>>(MODE_KEY).and_then(|m| m.lock().ok().map(|m| *m)).unwrap_or_default()
}

/// A Library guide read to its end (the Library page, through the engine): the next tick applies
/// its bonus to every skill whose current stage it was written for.
pub fn note_guide_read(data: &DataStore, slug: &str) {
    if let Some(Ok(mut q)) = data.get::<std::sync::Mutex<Vec<String>>>(GUIDE_READS_KEY).map(|m| m.lock()) {
        q.push(slug.to_string());
    }
}

/// The people near the player who could teach, replaced each frame.
pub fn publish_helpers(data: &DataStore, helpers: Vec<Helper>) {
    if let Some(Ok(mut h)) = data.get::<std::sync::Mutex<Vec<Helper>>>(HELPERS_KEY).map(|m| m.lock()) {
        *h = helpers;
    }
}

/// What reading guides did since the last call (drained).
pub fn drain_notices(data: &DataStore) -> Vec<GuideNotice> {
    data.get::<std::sync::Mutex<Vec<GuideNotice>>>(NOTICES_KEY)
        .and_then(|m| m.lock().ok().map(|mut n| std::mem::take(&mut *n)))
        .unwrap_or_default()
}

/// Variety: how repeats of one task lose value and recover.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Variety {
    /// Attempts at one task inside a rest window that count in full (the first among them).
    pub full_attempts: u32,
    /// Each attempt after those is worth this times the one before.
    pub per_repeat: f32,
    /// Never worth less than this.
    pub floor: f32,
    /// An attempt stops counting this many in-game days after it was made.
    pub rest_days: f64,
    /// The very first attempt at a task never done before is worth this many times more.
    pub first_time: f32,
}

impl Variety {
    /// What attempt number `attempt` (1 = the first in the rest window) is worth, with `full`
    /// attempts counting in full: `per_repeat` to the power of the attempts past `full`, never
    /// below `floor`.
    pub fn factor(&self, attempt: u32, full: u32) -> f32 {
        let past = attempt.saturating_sub(full.max(1));
        self.per_repeat.clamp(0.0, 1.0).powi(past.min(1000) as i32).max(self.floor.clamp(0.0, 1.0))
    }
}

/// Help from reading the Library guide for your stage.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct GuideHelp {
    pub multiplier: f32,
    /// In-game hours it lasts (an hour is always 3,600 game seconds, crate::time).
    pub hours: f64,
}

/// Help from practising beside someone well above you.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct MentorHelp {
    pub multiplier: f32,
    /// How many stages above you they must be.
    pub stages_above: usize,
    /// How near, in metres.
    pub reach_m: f32,
    /// How long the screen keeps saying so after a practice it helped (game seconds).
    pub shown_s: f64,
}

/// One level's rules: each part None is off at that level.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct LevelRules {
    #[serde(default)]
    pub variety: Option<Variety>,
    #[serde(default)]
    pub guide: Option<GuideHelp>,
    #[serde(default)]
    pub mentor: Option<MentorHelp>,
}

/// What counts as reading a guide: its end reached after at least this long.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Reading {
    pub seconds_per_100_words: f32,
    pub min_seconds: f32,
}

impl Reading {
    /// Seconds a guide of `words` words must be open before reaching its end counts as reading it.
    pub fn seconds_for(&self, words: usize) -> f32 {
        (words as f32 / 100.0 * self.seconds_per_100_words).max(self.min_seconds)
    }
}

#[derive(Deserialize)]
struct QualityFile {
    levels: HashMap<String, LevelRules>,
    reading: Reading,
}

#[derive(Deserialize)]
struct GuidesFile {
    guides: HashMap<String, HashMap<String, String>>,
}

/// data/skills/practice_quality.json and data/skills/guides.json, loaded.
#[derive(Debug, Clone)]
pub struct PracticeQuality {
    /// Level id ("off", "simplified", "realistic") to its rules.
    pub levels: HashMap<String, LevelRules>,
    pub reading: Reading,
    /// Skill id, then stage id, to the Library slug of the guide written for that stage.
    pub guides: HashMap<String, HashMap<String, String>>,
}

impl PracticeQuality {
    /// Both files' bytes (`guides` may be empty: no guides yet).
    pub fn from_json(quality: &[u8], guides: &[u8]) -> Result<Self, String> {
        let q: QualityFile = serde_json::from_slice(quality).map_err(|e| format!("data/{FILE}: {e}"))?;
        let guides = Self::guides_from_json(guides)?;
        for id in ["off", "simplified", "realistic"] {
            if !q.levels.contains_key(id) {
                return Err(format!("data/{FILE}: no `{id}` level"));
            }
        }
        Ok(Self { levels: q.levels, reading: q.reading, guides })
    }

    /// data/skills/guides.json's map (skill, then stage, to a Library slug); empty bytes, no guides.
    pub fn guides_from_json(guides: &[u8]) -> Result<HashMap<String, HashMap<String, String>>, String> {
        if guides.is_empty() {
            return Ok(HashMap::new());
        }
        Ok(serde_json::from_slice::<GuidesFile>(guides).map_err(|e| format!("data/{GUIDES_FILE}: {e}"))?.guides)
    }

    /// The rules at `mode` (none at a level the file lacks).
    pub fn rules(&self, mode: LearningMode) -> LevelRules {
        self.levels.get(mode.id()).cloned().unwrap_or_default()
    }

    /// The Library slug of the guide written for `stage` of `skill`.
    pub fn guide_for(&self, skill: &str, stage: &str) -> Option<&str> {
        self.guides.get(skill).and_then(|g| g.get(stage)).map(String::as_str)
    }
}

/// Someone near the player who could teach: a crew member with skills in crew.ron (players join
/// when the presence sync carries their stages, a later step).
#[derive(Debug, Clone, PartialEq)]
pub struct Helper {
    pub name: String,
    /// From the player, in metres.
    pub distance_m: f32,
    /// Skill id to the stage id they are at.
    pub stages: HashMap<String, String>,
}

/// A guide read for a stage: when, and until when its bonus lasts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuideRead {
    pub slug: String,
    pub at: f64,
    pub until: f64,
}

/// What reading a guide did, for the screen.
#[derive(Debug, Clone, PartialEq)]
pub enum GuideNotice {
    /// The bonus started: `skill`'s practice counts `multiplier` times until game second `until`.
    Applied { skill: String, stage: String, multiplier: f32, until: f64 },
    /// Read before at this stage: nothing more (once per guide per stage).
    AlreadyRead { skill: String, stage: String },
}

/// Why one attempt gave what it gave (the Skills page's explanation; not saved).
#[derive(Debug, Clone, PartialEq)]
pub struct PracticeWhy {
    /// The task (verb and thing): "collect.wood_log_0", "craft.bread", "harvest.tomato".
    pub task: String,
    /// The XP the verb names, before any weighing.
    pub base: u32,
    /// What it gave.
    pub gained: u32,
    /// Game second it happened.
    pub at: f64,
    pub mode: LearningMode,
    /// Its number at this task in the rest window (1 = the first), 0 with variety off.
    pub attempt: u32,
    /// Attempts that count in full in a window, for this task.
    pub full_attempts: u32,
    /// The first attempt at this task ever: its bonus multiplier (1 when it was not the first).
    pub first_time: f32,
    /// What repeating it made it worth (1 = full).
    pub variety: f32,
    /// The guide bonus applied: the stage, the guide's slug, the multiplier.
    pub guide: Option<(String, String, f32)>,
    /// The teacher beside the player: their name, their stage, the multiplier.
    pub mentor: Option<(String, String, f32)>,
    /// Rung 1's weights: difficulty against the level (and the task's difficulty less the level,
    /// when it has one), the result's quality, a declared transfer.
    pub difficulty: f32,
    pub difficulty_gap: Option<i64>,
    pub quality: f32,
    pub transfer: f32,
}

/// A teacher's help being applied now (the HUD says so while they stay within reach).
#[derive(Debug, Clone, PartialEq)]
pub struct Beside {
    pub name: String,
    pub skill: String,
    pub stage: String,
    pub multiplier: f32,
    /// Game second the HUD stops saying so unless another practice it helps comes first.
    pub until: f64,
}

/// One attempt, as variety and help weigh it.
#[derive(Debug, Clone, PartialEq)]
pub struct Weighed {
    pub variety: f32,
    pub attempt: u32,
    pub full_attempts: u32,
    /// The first-time bonus (1 when it was not the first attempt ever).
    pub first_time: f32,
    pub guide: Option<(String, String, f32)>,
    pub mentor: Option<(String, String, f32)>,
}

impl Weighed {
    /// The multiplier variety and help give together.
    pub fn factor(&self) -> f32 {
        self.variety * self.first_time * self.guide.as_ref().map_or(1.0, |g| g.2) * self.mentor.as_ref().map_or(1.0, |m| m.2)
    }
}

/// The player's memory of practice: each task's recent attempts, every task ever done, each guide
/// read per stage. Saved with the player (`WorldSave::practice`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PracticeMemory {
    /// "skill|task" to the game seconds of its attempts inside the rest window (oldest first).
    #[serde(default)]
    pub recent: BTreeMap<String, Vec<f64>>,
    /// Every "skill|task" ever practised (the first-time bonus).
    #[serde(default)]
    pub done: BTreeSet<String>,
    /// "skill|stage" to the guide read at that stage.
    #[serde(default)]
    pub guides_read: BTreeMap<String, GuideRead>,
    /// Skill id to why its last attempt gave what it gave (for the Skills page; not saved).
    #[serde(skip)]
    pub last: HashMap<String, PracticeWhy>,
    /// Teachers helping now (for the HUD; not saved).
    #[serde(skip)]
    pub beside: Vec<Beside>,
    /// Every attempt weighed this run, skill and why, oldest first, at most `TRIES_MAX` (not saved):
    /// the headless soak reads what its routine practised from here. `tried` counts them all, so a
    /// reader knows which it has already seen.
    #[serde(skip)]
    pub tries: std::collections::VecDeque<(String, PracticeWhy)>,
    #[serde(skip)]
    pub tried: u64,
}

/// Attempts kept in `PracticeMemory::tries`.
const TRIES_MAX: usize = 256;

/// Attempts kept per task: past the floor more change nothing, so a long day stops growing the save.
const RECENT_MAX: usize = 200;

/// A game hour (crate::time: an hour is always 3,600 game seconds).
const HOUR_S: f64 = 3600.0;

/// One attempt to weigh: whose stage, at what, when.
#[derive(Debug, Clone, Copy)]
pub struct Attempt<'a> {
    pub skill: &'a str,
    pub task: &'a str,
    /// The player's stage in the skill (an id of stages.json: "untrained" .. "master").
    pub stage: &'a str,
    /// Its place in the stage order (0 = Untrained).
    pub stage_index: usize,
    /// Game seconds now.
    pub now: f64,
    /// Game seconds in one day (crate::time::GameTime::seconds_per_day).
    pub day_s: f64,
    /// The verb's own `full_attempts` (data/skills/practice.json), when it sets one.
    pub full_override: Option<u32>,
}

impl PracticeMemory {
    /// Remember why an attempt at `skill` gave what it gave: the Skills page's last line for it,
    /// and the run's list of attempts.
    pub fn note_try(&mut self, skill: &str, why: PracticeWhy) {
        self.last.insert(skill.to_string(), why.clone());
        self.tries.push_back((skill.to_string(), why));
        while self.tries.len() > TRIES_MAX {
            self.tries.pop_front();
        }
        self.tried += 1;
    }

    /// The attempts after the first `seen` of the run (as many as are still kept), and the count
    /// to pass next time.
    pub fn tries_since(&self, seen: u64) -> (Vec<&(String, PracticeWhy)>, u64) {
        let new = self.tried.saturating_sub(seen).min(self.tries.len() as u64) as usize;
        (self.tries.iter().skip(self.tries.len() - new).collect(), self.tried)
    }

    /// Weigh one attempt by variety and help at `rules`, and remember it. `helpers` are the people
    /// near the player; `stage_ids` the stage order (to compare a helper's stage with the player's).
    pub fn weigh(&mut self, rules: &LevelRules, a: &Attempt, helpers: &[Helper], stage_ids: &[String]) -> Weighed {
        let key = format!("{}|{}", a.skill, a.task);
        let mut w = Weighed { variety: 1.0, attempt: 0, full_attempts: 0, first_time: 1.0, guide: None, mentor: None };
        if let Some(v) = &rules.variety {
            let rest = (v.rest_days.max(0.0) * a.day_s.max(1.0)).max(1.0);
            let times = self.recent.entry(key.clone()).or_default();
            times.retain(|t| a.now - *t < rest && *t <= a.now);
            let full = a.full_override.unwrap_or(v.full_attempts).max(1);
            w.attempt = times.len() as u32 + 1;
            w.full_attempts = full;
            w.variety = v.factor(w.attempt, full);
            if !self.done.contains(&key) {
                w.first_time = v.first_time.max(1.0);
            }
            times.push(a.now);
            if times.len() > RECENT_MAX {
                let extra = times.len() - RECENT_MAX;
                times.drain(..extra);
            }
            self.done.insert(key);
        }
        if let Some(g) = &rules.guide {
            if let Some(read) = self.guides_read.get(&format!("{}|{}", a.skill, a.stage)) {
                if a.now < read.until {
                    w.guide = Some((a.stage.to_string(), read.slug.clone(), g.multiplier.max(1.0)));
                }
            }
        }
        if let Some(m) = &rules.mentor {
            let best = helpers
                .iter()
                .filter(|h| h.distance_m <= m.reach_m)
                .filter_map(|h| {
                    let stage = h.stages.get(a.skill)?;
                    let idx = stage_ids.iter().position(|s| s == stage)?;
                    (idx >= a.stage_index + m.stages_above).then_some((h, stage, idx))
                })
                .max_by_key(|(_, _, idx)| *idx);
            if let Some((h, stage, _)) = best {
                w.mentor = Some((h.name.clone(), stage.clone(), m.multiplier.max(1.0)));
                self.beside.retain(|b| !(b.name == h.name && b.skill == a.skill));
                self.beside.push(Beside {
                    name: h.name.clone(),
                    skill: a.skill.to_string(),
                    stage: stage.clone(),
                    multiplier: m.multiplier.max(1.0),
                    until: a.now + m.shown_s.max(0.0),
                });
            }
        }
        w
    }

    /// A Library guide `slug` was read to its end at game second `now`: for every skill whose
    /// stage the player is at (`stage_of(skill)`) has `slug` as its guide, the bonus starts, once
    /// per guide per stage. What happened, per skill.
    pub fn read_guide(
        &mut self,
        quality: &PracticeQuality,
        rules: &LevelRules,
        slug: &str,
        stage_of: impl Fn(&str) -> Option<String>,
        now: f64,
    ) -> Vec<GuideNotice> {
        let Some(g) = &rules.guide else { return Vec::new() };
        let mut skills: Vec<(&String, &HashMap<String, String>)> = quality.guides.iter().collect();
        skills.sort_by(|a, b| a.0.cmp(b.0));
        let mut out = Vec::new();
        for (skill, stages) in skills {
            let Some(stage) = stage_of(skill) else { continue };
            if stages.get(&stage).map(String::as_str) != Some(slug) {
                continue;
            }
            let key = format!("{skill}|{stage}");
            if self.guides_read.contains_key(&key) {
                out.push(GuideNotice::AlreadyRead { skill: skill.clone(), stage });
                continue;
            }
            let until = now + g.hours.max(0.0) * HOUR_S;
            self.guides_read.insert(key, GuideRead { slug: slug.to_string(), at: now, until });
            out.push(GuideNotice::Applied { skill: skill.clone(), stage, multiplier: g.multiplier.max(1.0), until });
        }
        out
    }

    /// The guide bonus running for `skill` at `stage` now: its slug and when it ends.
    pub fn guide_bonus(&self, skill: &str, stage: &str, now: f64) -> Option<&GuideRead> {
        self.guides_read.get(&format!("{skill}|{stage}")).filter(|r| now < r.until)
    }

    /// Whether the guide for `skill` at `stage` has been read (its bonus used or running).
    pub fn guide_read(&self, skill: &str, stage: &str) -> bool {
        self.guides_read.contains_key(&format!("{skill}|{stage}"))
    }

    /// Teachers whose help the HUD should still name: not past their time and still within reach.
    pub fn beside_now(&self, now: f64, helpers: &[Helper], reach_m: f32) -> Vec<&Beside> {
        self.beside
            .iter()
            .filter(|b| now < b.until && helpers.iter().any(|h| h.name == b.name && h.distance_m <= reach_m))
            .collect()
    }
}

/// Everything the SkillSystem needs to weigh this tick's attempts: the table, the level, the
/// people near, the clock.
pub struct Context<'a> {
    pub quality: Option<&'a PracticeQuality>,
    pub mode: LearningMode,
    pub rules: LevelRules,
    pub helpers: Vec<Helper>,
    pub now: f64,
    pub day_s: f64,
}

impl<'a> Context<'a> {
    /// Read from the DataStore (the table absent: no variety or help, as in rung 1).
    pub fn of(data: &'a DataStore) -> Self {
        let quality = data.get::<PracticeQuality>(DATA_KEY);
        let mode = mode_of(data);
        let rules = quality.map(|q| q.rules(mode)).unwrap_or_default();
        let helpers = data.get::<std::sync::Mutex<Vec<Helper>>>(HELPERS_KEY).and_then(|m| m.lock().ok().map(|h| h.clone())).unwrap_or_default();
        let (now, day_s) = data
            .get::<std::sync::Mutex<crate::time::GameTime>>("game_time")
            .and_then(|m| m.lock().ok().map(|g| (g.elapsed_seconds, g.seconds_per_day())))
            .unwrap_or((0.0, 24.0 * HOUR_S));
        Self { quality, mode, rules, helpers, now, day_s }
    }
}

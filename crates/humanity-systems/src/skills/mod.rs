//! Skill progression: learn by doing the real procedure, named stages, no cap.
//!
//! The base set (docs/design/skills.md, approved by the operator 2026-11-14) is data:
//! - `data/skills/skills.csv`: one row per skill (id, family, XP base, declared transfers, and the
//!   reason when no activity trains it yet). Names and descriptions are words by key:
//!   `skills.<id>.name` and `skills.<id>.desc` in `data/i18n/en.json`.
//! - `data/skills/families.json`: the families, in the order the Skills page shows them.
//! - `data/skills/stages.json`: the six named stages (Novice to Master, plus Untrained) and every
//!   skill's ladder: the level each stage starts at and the keys of its "I can" statements.
//! - `data/skills/practice.json`: what each live verb teaches (`practice`, `practice_distance`).
//!
//! Levels have no top: the XP curve (`base x level^1.5`, `data/skills/xp_curve.json`) keeps going
//! past Master, and a stage is the band of levels between one stage's `from_level` and the next.
//! XP for one attempt is weighed by difficulty against the person's level and by the quality of
//! the result (`practice_weight`, skills.md section 3); a declared transfer from a skill the person
//! is better at speeds learning (section 1, rule 5).

use humanity_core::ecs::systems::System;
use humanity_core::hot_reload::data_store::DataStore;
use serde::Deserialize;
use std::collections::HashMap;

/// One row of `data/skills/skills.csv`.
#[derive(Debug, Clone, Deserialize)]
struct SkillRow {
    id: String,
    family: String,
    xp_per_level: u32,
    #[serde(default)]
    transfers: String,
    #[serde(default)]
    untrained_reason: String,
}

/// One named stage of one skill: where it starts and what a person at it can do.
#[derive(Debug, Clone, Deserialize)]
pub struct StageStep {
    /// A stage id from `data/skills/stages.json` ("novice" .. "master").
    pub stage: String,
    /// The level this stage starts at (the numbers underneath the words).
    pub from_level: u32,
    /// The i18n keys of its "I can" statements.
    pub can: Vec<String>,
}

/// A skill: its row, plus its ladder of stages from `stages.json`.
#[derive(Debug, Clone)]
pub struct SkillDef {
    pub id: String,
    /// A family id from `data/skills/families.json`.
    pub family: String,
    /// The XP base: going from level L-1 to L costs `xp_per_level x L^1.5`.
    pub xp_per_level: u32,
    /// Declared transfers to other skills: (skill id, head start fraction).
    pub transfers: Vec<(String, f32)>,
    /// Why no activity trains it yet ("" when one does).
    pub untrained_reason: String,
    /// Novice to Master, in order (empty when only skills.csv was loaded).
    pub ladder: Vec<StageStep>,
}

impl SkillDef {
    /// XP needed to go from `level - 1` to `level` (base x level^1.5, rounded down). No top level.
    pub fn xp_for_level(&self, level: u32) -> u32 {
        xp_for_level_at(self.xp_per_level, level)
    }

    /// The i18n key of its name.
    pub fn name_key(&self) -> String {
        format!("skills.{}.name", self.id)
    }

    /// The i18n key of its plain description.
    pub fn desc_key(&self) -> String {
        format!("skills.{}.desc", self.id)
    }

    /// Its name in the person's language (the key itself when no language has it). The server
    /// build has no word source, so there it is the id in plain spaces.
    #[cfg(feature = "native")]
    pub fn name(&self) -> String {
        humanity_core::i18n::t_dyn(&self.name_key()).to_string()
    }
    #[cfg(not(feature = "native"))]
    pub fn name(&self) -> String {
        self.id.replace('_', " ")
    }

    /// How many stages of the ladder `level` has reached: 0 = Untrained, 1 = Novice ...
    pub fn stage_index(&self, level: u32) -> usize {
        self.ladder.iter().take_while(|s| level >= s.from_level).count()
    }

    /// The stage `level` is in (None while Untrained).
    pub fn stage_at(&self, level: u32) -> Option<&StageStep> {
        self.stage_index(level).checked_sub(1).and_then(|i| self.ladder.get(i))
    }

    /// The next stage after `level`'s (None at Master: the bar keeps filling, there is no top).
    pub fn next_stage(&self, level: u32) -> Option<&StageStep> {
        self.ladder.get(self.stage_index(level))
    }

    /// The level the Master stage starts at: where the quality a level gives stops rising
    /// (crafting::quality::skill_factor) and what the dev "every skill to Master" button sets.
    /// 1 when no ladder is loaded.
    pub fn mastery_level(&self) -> u32 {
        self.ladder.last().map_or(1, |s| s.from_level.max(1))
    }

    /// Progress from the start of `level`'s stage to the next stage, 0..1, counting the XP already
    /// earned toward the next level. 1.0 at Master.
    pub fn stage_progress(&self, level: u32, xp: u32) -> f32 {
        let Some(next) = self.next_stage(level) else { return 1.0 };
        let start = self.stage_at(level).map_or(0, |s| s.from_level);
        let need: u64 = (start + 1..=next.from_level).map(|l| self.xp_for_level(l) as u64).sum();
        let done: u64 = (start + 1..=level).map(|l| self.xp_for_level(l) as u64).sum::<u64>() + xp as u64;
        if need == 0 {
            return 1.0;
        }
        (done as f32 / need as f32).clamp(0.0, 1.0)
    }
}

/// XP to go from `level - 1` to `level` for a skill of XP base `base`: base x level^1.5.
pub fn xp_for_level_at(base: u32, level: u32) -> u32 {
    (base as f64 * (level as f64).powf(1.5)) as u32
}

/// How much one attempt teaches, as a multiplier on its base XP (docs/design/skills.md section 3,
/// "What counts as practice"):
/// - **difficulty against your level** (`difficulty`, in levels, a recipe's `skill_level`): work at
///   your level or a little above teaches most (x1.25), one level below x1.0, and each level further
///   below three quarters of that, down to x0.1, so repeating the cheapest recipe stops paying;
///   far above (more than 2 levels) teaches a little, mostly from the failure (x0.75);
/// - **the quality of the result** (`quality`, 0..1 as the simulation grades it): x0.5 for the
///   worst to x1.5 for the best, so a careful good result teaches three times a careless one.
/// None for either leaves it out (x1.0). Variety and help (a guide read, a teacher beside you) are
/// later rungs (skills.md section 10).
pub fn practice_weight(level: u32, difficulty: Option<u32>, quality: Option<f32>) -> f32 {
    let by_difficulty = match difficulty {
        None => 1.0,
        Some(d) => {
            let gap = d as i64 - level as i64;
            if gap > 2 {
                0.75
            } else if gap >= 0 {
                1.25
            } else {
                (0.75f32.powi((-gap - 1) as i32)).max(0.1)
            }
        }
    };
    let by_quality = quality.map_or(1.0, |q| 0.5 + q.clamp(0.0, 1.0));
    by_difficulty * by_quality
}

/// One verb's teaching, from `data/skills/practice.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct Practice {
    /// The skills it trains (empty: the caller names the skill from data, the weapon's
    /// `skill` column for a landed strike).
    #[serde(default)]
    pub skills: Vec<String>,
    /// Base XP per attempt (or per `per_metres`).
    pub xp: u32,
    /// For a continuous verb (swimming, climbing, driving, carrying): one attempt per this many
    /// metres done.
    #[serde(default)]
    pub per_metres: Option<f32>,
}

#[derive(Deserialize)]
struct PracticeFile {
    practices: HashMap<String, Practice>,
}

#[derive(Deserialize)]
struct StagesFile {
    stages: Vec<StageName>,
    ladders: HashMap<String, Vec<StageStep>>,
}

#[derive(Deserialize)]
struct StageName {
    id: String,
}

#[derive(Deserialize)]
struct FamiliesFile {
    families: Vec<FamilyId>,
}

#[derive(Deserialize)]
struct FamilyId {
    id: String,
}

/// Player's progress in a single skill.
#[derive(Debug, Clone)]
pub struct SkillProgress {
    pub level: u32,
    pub xp: u32,
}

/// Component tracking all skills for an entity.
pub struct PlayerSkills {
    pub skills: HashMap<String, SkillProgress>,
}

impl PlayerSkills {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// Get current level for a skill.
    pub fn level(&self, skill_id: &str) -> u32 {
        self.skills.get(skill_id).map(|s| s.level).unwrap_or(0)
    }

    /// Get current XP for a skill.
    pub fn xp(&self, skill_id: &str) -> u32 {
        self.skills.get(skill_id).map(|s| s.xp).unwrap_or(0)
    }
}

/// Registry of all skill definitions, their families and stages, and what each verb teaches.
pub struct SkillRegistry {
    pub skills: HashMap<String, SkillDef>,
    /// Family ids in page order (`data/skills/families.json`).
    pub families: Vec<String>,
    /// Stage ids in order, Untrained first (`data/skills/stages.json`).
    pub stage_ids: Vec<String>,
    /// What each verb teaches (`data/skills/practice.json`).
    pub practices: HashMap<String, Practice>,
    /// Metres done toward the next attempt of each continuous verb (in memory only, rebuilt from
    /// zero each run: nothing here can go stale).
    meters: std::sync::Mutex<HashMap<String, f32>>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
            families: Vec::new(),
            stage_ids: Vec::new(),
            practices: HashMap::new(),
            meters: std::sync::Mutex::new(HashMap::new()),
        }
    }

    pub fn register(&mut self, def: SkillDef) {
        self.skills.insert(def.id.clone(), def);
    }

    pub fn get(&self, id: &str) -> Option<&SkillDef> {
        self.skills.get(id)
    }

    /// Build the registry from raw `data/skills/skills.csv` bytes (no ladders, families or
    /// practices yet: `with_stages`, `with_families` and `with_practices` add them). This is what
    /// populates `DataStore["skill_registry"]`; without it every XP award silently no-ops.
    pub fn from_csv(data: &[u8]) -> Result<Self, String> {
        let rows: Vec<SkillRow> = humanity_core::data_parse::parse_csv(data)?;
        let mut reg = Self::new();
        for r in rows {
            let transfers = r
                .transfers
                .split('|')
                .filter(|t| !t.trim().is_empty())
                .map(|t| {
                    let (id, f) = t.split_once(':').ok_or_else(|| format!("skill {}: transfer `{t}` is not id:fraction", r.id))?;
                    let f: f32 = f.trim().parse().map_err(|_| format!("skill {}: transfer `{t}` has no number", r.id))?;
                    Ok((id.trim().to_string(), f))
                })
                .collect::<Result<Vec<_>, String>>()?;
            reg.register(SkillDef {
                id: r.id,
                family: r.family,
                xp_per_level: r.xp_per_level,
                transfers,
                untrained_reason: r.untrained_reason,
                ladder: Vec::new(),
            });
        }
        Ok(reg)
    }

    /// Add every skill's ladder from `data/skills/stages.json`.
    pub fn with_stages(mut self, json: &[u8]) -> Result<Self, String> {
        let f: StagesFile = serde_json::from_slice(json).map_err(|e| format!("data/skills/stages.json: {e}"))?;
        self.stage_ids = f.stages.into_iter().map(|s| s.id).collect();
        for (id, ladder) in f.ladders {
            if let Some(def) = self.skills.get_mut(&id) {
                def.ladder = ladder;
            }
        }
        Ok(self)
    }

    /// Add the family order from `data/skills/families.json`.
    pub fn with_families(mut self, json: &[u8]) -> Result<Self, String> {
        let f: FamiliesFile = serde_json::from_slice(json).map_err(|e| format!("data/skills/families.json: {e}"))?;
        self.families = f.families.into_iter().map(|x| x.id).collect();
        Ok(self)
    }

    /// Add what each verb teaches from `data/skills/practice.json`.
    pub fn with_practices(mut self, json: &[u8]) -> Result<Self, String> {
        let f: PracticeFile = serde_json::from_slice(json).map_err(|e| format!("data/skills/practice.json: {e}"))?;
        self.practices = f.practices;
        Ok(self)
    }

    /// The largest declared head start into `skill_id` from a skill the player is better at.
    fn transfer_bonus(&self, skills: &PlayerSkills, skill_id: &str) -> f32 {
        let mine = skills.level(skill_id);
        self.skills
            .values()
            .flat_map(|d| d.transfers.iter().filter(|(to, _)| to == skill_id).map(move |(_, f)| (d.id.as_str(), *f)))
            .filter(|(from, _)| skills.level(from) > mine)
            .map(|(_, f)| f)
            .fold(0.0, f32::max)
    }
}

/// XP gain event queued for processing.
#[derive(Debug, Clone)]
pub struct SkillXPEvent {
    pub skill_id: String,
    pub amount: u32,
    /// The attempt's difficulty in levels (a recipe's skill_level), when it has one.
    pub difficulty: Option<u32>,
    /// The result's quality 0..1, when the simulation grades it.
    pub quality: Option<f32>,
}

impl SkillXPEvent {
    /// A plain grant: no difficulty, no quality.
    pub fn plain(skill_id: impl Into<String>, amount: u32) -> Self {
        Self { skill_id: skill_id.into(), amount, difficulty: None, quality: None }
    }
}

/// Push a skill-XP grant onto the shared `"xp_grants"` DataStore channel that
/// [`SkillSystem`] drains each tick, unweighted (a quest reward, an ability cast). This is the
/// decoupled path any action system uses to award XP: they hold only `&DataStore` (not
/// `&mut SkillSystem`). No-ops cleanly if the channel is absent or the amount is zero.
pub fn award_skill_xp(data: &DataStore, skill_id: &str, amount: u32) {
    award_practice(data, skill_id, amount, None, None);
}

/// Push one attempt at a skill, weighed in the SkillSystem by its `difficulty` against the
/// player's level and the `quality` of the result (`practice_weight`).
pub fn award_practice(data: &DataStore, skill_id: &str, amount: u32, difficulty: Option<u32>, quality: Option<f32>) {
    if amount == 0 {
        return;
    }
    if let Some(lock) = data.get::<std::sync::Mutex<Vec<SkillXPEvent>>>("xp_grants") {
        if let Ok(mut grants) = lock.lock() {
            grants.push(SkillXPEvent { skill_id: skill_id.to_string(), amount, difficulty, quality });
        }
    }
}

/// The player did the verb `id` once: every skill its `data/skills/practice.json` entry names gets
/// the entry's XP, weighed by `quality` when the verb grades the result. An id the table lacks
/// teaches nothing (tests/skills_data_lint.rs holds every id the code names to the table).
pub fn practice(data: &DataStore, id: &str, quality: Option<f32>) {
    let Some(reg) = data.get::<SkillRegistry>("skill_registry") else { return };
    let Some(p) = reg.practices.get(id) else {
        log::debug!("practice `{id}` is not in data/skills/practice.json; it teaches nothing");
        return;
    };
    for skill in &p.skills {
        award_practice(data, skill, p.xp, None, quality);
    }
}

/// `practice` with the caller's own XP amount (a harvest scales with what was picked): the
/// entry names the skills, `amount` replaces its `xp`.
pub fn practice_scaled(data: &DataStore, id: &str, amount: u32, quality: Option<f32>) {
    let Some(reg) = data.get::<SkillRegistry>("skill_registry") else { return };
    if let Some(p) = reg.practices.get(id) {
        for skill in &p.skills {
            award_practice(data, skill, amount, None, quality);
        }
    }
}

/// `practice` for a verb whose skill comes from data the caller holds (a landed strike trains the
/// weapon's `skill` column): the entry gives the XP, `skill` the skill.
pub fn practice_with(data: &DataStore, id: &str, skill: &str, quality: Option<f32>) {
    let Some(reg) = data.get::<SkillRegistry>("skill_registry") else { return };
    if let Some(p) = reg.practices.get(id) {
        award_practice(data, skill, p.xp, None, quality);
    }
}

/// The first of `ids` the practice table has (a specific entry, then its general one:
/// "build.stone_wall" before "build"), practised once.
pub fn practice_first(data: &DataStore, ids: &[&str], quality: Option<f32>) {
    let Some(reg) = data.get::<SkillRegistry>("skill_registry") else { return };
    if let Some(id) = ids.iter().find(|id| reg.practices.contains_key(**id)) {
        practice(data, id, quality);
    }
}

/// A continuous verb (swimming, climbing, driving, carrying) covered `metres`: every
/// `per_metres` of its entry is one attempt. Metres below that carry over (in memory only).
pub fn practice_distance(data: &DataStore, id: &str, metres: f32, quality: Option<f32>) {
    if !(metres > 0.0) {
        return;
    }
    let Some(reg) = data.get::<SkillRegistry>("skill_registry") else { return };
    let Some(p) = reg.practices.get(id) else { return };
    let per = p.per_metres.unwrap_or(1.0).max(0.01);
    let attempts = {
        let Ok(mut meters) = reg.meters.lock() else { return };
        let done = meters.entry(id.to_string()).or_insert(0.0);
        *done += metres;
        let n = (*done / per).floor();
        *done -= n * per;
        n as u32
    };
    for _ in 0..attempts.min(20) {
        for skill in &p.skills {
            award_practice(data, skill, p.xp, None, quality);
        }
    }
}

/// Practice done on a page that holds no DataStore (a vote cast on the Governance page, an
/// organisation joined): queued here and taught by the SkillSystem's next tick. In memory only.
static QUEUED_PRACTICE: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());

/// Queue the verb `id` from a page with no DataStore; `practice` runs it on the next tick.
pub fn queue_practice(id: &'static str) {
    if let Ok(mut q) = QUEUED_PRACTICE.lock() {
        q.push(id);
    }
}

/// Skill system processes XP events and levels up skills.
pub struct SkillSystem {
    pending_xp: Vec<SkillXPEvent>,
    level_ups: Vec<(String, u32)>,
}

impl SkillSystem {
    pub fn new() -> Self {
        Self {
            pending_xp: Vec::new(),
            level_ups: Vec::new(),
        }
    }

    /// Queue an XP gain event (unweighted).
    pub fn award_xp(&mut self, skill_id: String, amount: u32) {
        self.pending_xp.push(SkillXPEvent::plain(skill_id, amount));
    }

    /// Get and clear recent level-ups (for UI notifications).
    pub fn drain_level_ups(&mut self) -> Vec<(String, u32)> {
        self.level_ups.drain(..).collect()
    }
}

impl System for SkillSystem {
    fn name(&self) -> &str {
        "Skills"
    }

    fn tick(&mut self, world: &mut hecs::World, _dt: f32, data: &DataStore) {
        // XP arrives two ways: direct award_xp() calls (used by tests) and the
        // shared "xp_grants" DataStore channel that the action systems
        // (crafting/farming/mining) push to — they hold only `&DataStore`, not
        // `&mut SkillSystem`, so the channel is the decoupling. SkillSystem is
        // registered LAST in the runner, so grants pushed earlier THIS frame are
        // drained + applied the same frame.
        let registry = data.get::<SkillRegistry>("skill_registry");

        // Dev: every skill to Master (testing affordance — preserves the "develop as if 100%
        // unlocked" posture even with skill-gated crafting). There is no top level, so "max" is
        // the level each skill's Master stage starts at. Drains the dev_max_skills bool channel
        // (set by the profile Skills panel's button).
        let dev_max = data
            .get::<std::sync::Mutex<bool>>("dev_max_skills")
            .and_then(|m| m.lock().ok().map(|mut s| std::mem::replace(&mut *s, false)))
            .unwrap_or(false);
        if dev_max {
            if let Some(reg) = registry {
                for (_e, skills) in world.query_mut::<&mut PlayerSkills>() {
                    for (id, def) in reg.skills.iter() {
                        let top = def.mastery_level().max(skills.level(id));
                        skills.skills.insert(id.clone(), SkillProgress { level: top, xp: 0 });
                    }
                    break;
                }
                log::info!("Dev: every player skill to Master");
            }
        }

        // Saved progress in a skill id that is no longer a row (the twenty old ids retired by
        // skills.md section 6: shelter_building, metalworking, engineering, melee, ranged,
        // defense) is dropped, not mapped: nobody plays yet (CLAUDE.md, no backwards
        // compatibility before launch), and a save keeps only what the base set names. Guarded so
        // a registry that failed to load never wipes anyone's skills.
        if let Some(reg) = registry.filter(|r| !r.skills.is_empty()) {
            for (_e, skills) in world.query_mut::<&mut PlayerSkills>() {
                skills.skills.retain(|id, _| reg.skills.contains_key(id));
            }
        }

        let queued: Vec<&'static str> = QUEUED_PRACTICE.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default();
        for id in queued {
            practice(data, id, None);
        }
        let mut events: Vec<SkillXPEvent> = self.pending_xp.drain(..).collect();
        if let Some(lock) = data.get::<std::sync::Mutex<Vec<SkillXPEvent>>>("xp_grants") {
            if let Ok(mut grants) = lock.lock() {
                events.append(&mut grants);
            }
        }
        if events.is_empty() {
            return;
        }
        let Some(reg) = registry else { return };

        // Find the entity with PlayerSkills (usually the local player)
        for (_entity, skills) in world.query_mut::<&mut PlayerSkills>() {
            for event in &events {
                let Some(def) = reg.get(&event.skill_id) else { continue };
                let level = skills.level(&event.skill_id);
                let weight = practice_weight(level, event.difficulty, event.quality)
                    * (1.0 + reg.transfer_bonus(skills, &event.skill_id));
                let gained = ((event.amount as f32 * weight).round() as u32).max(1);

                let progress = skills
                    .skills
                    .entry(event.skill_id.clone())
                    .or_insert(SkillProgress { level: 0, xp: 0 });
                progress.xp += gained;

                // Level up as often as the XP allows: no top level.
                loop {
                    let needed = def.xp_for_level(progress.level + 1);
                    if needed > 0 && progress.xp >= needed {
                        let stage_before = def.stage_index(progress.level);
                        progress.xp -= needed;
                        progress.level += 1;
                        self.level_ups.push((event.skill_id.clone(), progress.level));
                        if def.stage_index(progress.level) > stage_before {
                            let stage = def.stage_at(progress.level).map_or("", |s| s.stage.as_str());
                            log::info!("Skill '{}' reached the {} stage (level {})", def.id, stage, progress.level);
                        } else {
                            log::info!("Skill '{}' leveled up to {}!", def.id, progress.level);
                        }
                    } else {
                        break;
                    }
                }
            }
            break; // Only process first entity with PlayerSkills
        }
    }
}

/// The registry as the engine loads it (skills, ladders, families, practices), for tests anywhere
/// in the crate that need a verb to teach what the shipped table says.
#[cfg(test)]
pub(crate) fn shipped_for_tests() -> SkillRegistry {
    let read = |rel: &str| std::fs::read(format!("{}/data/skills/{rel}", env!("HUMANITY_REPO_ROOT"))).unwrap_or_default();
    SkillRegistry::from_csv(&read("skills.csv"))
        .and_then(|r| r.with_stages(&read("stages.json")))
        .and_then(|r| r.with_families(&read("families.json")))
        .and_then(|r| r.with_practices(&read("practice.json")))
        .expect("the shipped skill data loads")
}

#[cfg(test)]
mod skill_tests;

#[cfg(test)]
mod statement_review_tests;

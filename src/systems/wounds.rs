//! Wounds, and how a body heals (first-hour audit S8, 2026-10-12).
//!
//! Until 2026-10-12 nothing in the game left a wound and nothing healed by itself: a bite took
//! health and that was all, the Bleeding and Infected Wound effects existed and nothing gave
//! them, the medical catalogue `data/medical.ron` and its `MedicalSystem` were never run, and the
//! only health that came back on its own was Well Fed's 1 a second for half an hour after a
//! filling meal, which healed a body from nearly dead in under two minutes.
//!
//! Now:
//! - A creature's bite leaves Bleeding and a Bite Wound, and a fire leaves a burn whose depth
//!   follows how long you stood in it (red skin, blistered, deep). Each is a status effect of
//!   type `wound` (data/status_effects.csv) with a row in data/medical/wounds.ron saying what
//!   it does, how it passes, what helps, and what the player is told.
//! - Bleeding takes health for its few minutes and stops by itself, or at once with a Bandage
//!   or a Medkit. A wound that can go bad (a bite, a blistered or deep burn) may turn into an
//!   Infected Wound a day or two in, more often if nobody cleaned and covered it; Antibiotics
//!   end that. A Bandage, a Medkit or Antiseptic cleans and covers the wounds you have
//!   (`Wounds Cared For`, data/medical/treatments.ron `cares`).
//! - A body heals by itself (`Wounds::healing`): when nothing is harming it, slowly, about
//!   1.8 times as fast asleep, at half the rate while Hungry or Thirsty, and not at all while
//!   bleeding, infected or ill.
//!
//! ONE OWNER. The food system owns the body's meters (vitals and Health, with the exact
//! per-frame arithmetic of BUG-164), so it applies all of this in its body pass, the way it
//! applies illness (`systems::illness`) and the medical items (`systems::treatment`). This
//! module is the data and the rules. `MedicalSystem` was deleted with this change rather than
//! registered: it kept a second list of conditions beside `StatusEffects` (which is what the
//! save keeps, the Inventory shows and the medical items end), and it would have been a second
//! system changing Health every frame, in f32.
//!
//! HOW A WOUND ARRIVES. Whatever hurts a body pushes an `Injury` onto it (`injure`, which adds
//! the body's `Injuries` component): the combat system for a creature's bite, the fire system
//! for heat. The body pass takes them on its next tick (`Wounds::take_injuries`). The
//! component is not saved: an injury waits a frame at most, and a fire's exposure counts
//! seconds.
//!
//! THE CLOCK. A wound counts down, and does what it does, on the game clock, like an illness:
//! the time-speed setting and a night asleep move it along (`StatusEffectDef::on_game_clock`).
//! Its harm and the healing are worked out in f64 and reach Health through the food system's
//! `change_exactly` (BUG-164).
//!
//! TWO MODES. The illness setting (`illness::Mode`, Settings > Gameplay > Illness and injury):
//! Realistic uses the data's numbers, Forgiving its `forgiving` shares and faster healing.
//!
//! Pure data and rules with no engine state, so it compiles in every feature set.

use std::path::Path;

use serde::Deserialize;

use crate::ecs::components::StatusEffects;
use crate::systems::illness::{duration_words, Illnesses, Mode};
use crate::systems::status_effects::StatusEffectRegistry;

/// The `type` of status effect that is a wound: it runs on the game clock.
pub const WOUND: &str = "wound";

/// Seconds in an hour.
const HOUR_S: f64 = 3600.0;

/// Forgiving's shares of the Realistic numbers.
#[derive(Debug, Clone, Deserialize)]
pub struct Shares {
    /// Of a wound's course.
    pub course_share: f32,
    /// Of the harm a wound does.
    pub harm_share: f32,
    /// Of the chance a wound goes bad.
    pub goes_bad_share: f32,
}

/// How fast a body heals by itself (wounds.ron `healing`).
#[derive(Debug, Clone, Deserialize)]
pub struct Healing {
    /// Ordinary days (a night's sleep in each) from no health to full, Realistic.
    pub realistic_days: f32,
    /// The same, Forgiving.
    pub forgiving_days: f32,
    /// How many times as fast it heals asleep.
    pub asleep: f32,
    /// Effects that slow it (Hungry, Thirsty).
    #[serde(default)]
    pub slowed_by: Vec<String>,
    /// The share of the rate it heals at while one of them is on.
    pub slowed_share: f32,
}

/// The chance a wound goes bad (wounds.ron `goes_bad`).
#[derive(Debug, Clone, Deserialize)]
pub struct GoesBad {
    /// Game hours after it happened that it may.
    pub after_h: f32,
    /// The chance, when nobody cleaned and covered it.
    pub uncared: f32,
    /// The chance, when it was cleaned and covered by then.
    pub cared: f32,
    /// The wound it becomes (an Infected Wound).
    pub becomes: String,
}

/// One wound: a row of wounds.ron.
#[derive(Debug, Clone, Deserialize)]
pub struct Wound {
    /// The status effect it is (data/status_effects.csv, type `wound`).
    pub effect: String,
    /// Health it takes an hour of game time while it lasts, Realistic.
    #[serde(default)]
    pub health_per_h: f32,
    /// The body does not heal while it lasts.
    #[serde(default)]
    pub stops_healing: bool,
    /// The death screen's words when it is what killed the player.
    #[serde(default)]
    pub cause: String,
    /// Whether, and how likely, it goes bad.
    #[serde(default)]
    pub goes_bad: Option<GoesBad>,
    /// The HUD row's colour: 0 fine, 1 attention, 2 danger.
    #[serde(default)]
    pub severity: u8,
    /// Told when it happens.
    pub onset: String,
    /// How it passes, with `{time}` the time it lasts.
    pub course: String,
    /// What helps, in the game.
    pub helps: String,
    /// A few words for the HUD row.
    #[serde(default)]
    pub hud: String,
    /// Told when it has passed.
    pub healed: String,
}

/// One step of a burn's depth: from `from_s` seconds of fire at full intensity it is `effect`.
#[derive(Debug, Clone, Deserialize)]
pub struct BurnStep {
    pub from_s: f32,
    pub effect: String,
}

/// What a fire does to a body (wounds.ron `burns`).
#[derive(Debug, Clone, Deserialize)]
pub struct Burns {
    /// Health a second of fire takes at full intensity, Realistic.
    pub health_per_s: f32,
    /// The death screen's words.
    pub cause: String,
    /// Heat back after this many seconds away is a new burn.
    pub new_burn_after_s: f32,
    /// The depths, shallowest first.
    pub steps: Vec<BurnStep>,
}

/// Cleaning and covering wounds (wounds.ron `care`).
#[derive(Debug, Clone, Deserialize)]
pub struct Care {
    /// The status effect that says the wounds are cared for.
    pub effect: String,
    /// The Inventory line for a cared-for wound that can go bad.
    pub cared: String,
    /// The same for one nobody has cared for.
    pub uncared: String,
    /// The HUD's words for each.
    pub cared_hud: String,
    pub uncared_hud: String,
    /// Told when wounds are cared for.
    pub given: String,
    /// Told when a new wound arrives that the care does not cover.
    pub new_wound: String,
}

/// data/medical/wounds.ron.
#[derive(Debug, Clone, Deserialize)]
pub struct Wounds {
    pub forgiving: Shares,
    pub healing: Healing,
    pub care: Care,
    /// The wounds a creature's bite leaves.
    #[serde(default)]
    pub bite: Vec<String>,
    pub burns: Burns,
    #[serde(default)]
    pub wounds: Vec<Wound>,
}

impl Default for Wounds {
    /// No wound data: nothing leaves a wound and nothing heals by itself.
    fn default() -> Self {
        Self {
            forgiving: Shares { course_share: 1.0, harm_share: 1.0, goes_bad_share: 1.0 },
            healing: Healing { realistic_days: 0.0, forgiving_days: 0.0, asleep: 1.0, slowed_by: Vec::new(), slowed_share: 1.0 },
            care: Care {
                effect: String::new(),
                cared: String::new(),
                uncared: String::new(),
                cared_hud: String::new(),
                uncared_hud: String::new(),
                given: String::new(),
                new_wound: String::new(),
            },
            bite: Vec::new(),
            burns: Burns { health_per_s: 0.0, cause: String::new(), new_burn_after_s: 0.0, steps: Vec::new() },
            wounds: Vec::new(),
        }
    }
}

/// Something that hurt a body, waiting for the body pass to make it a wound (`injure`).
#[derive(Debug, Clone, PartialEq)]
pub enum Injury {
    /// A creature's bite (the combat system).
    Bite,
    /// Heat on the skin: seconds of fire at full intensity (the fire system).
    Heat { dose_s: f64 },
}

/// A body's injuries not yet taken, and the fire exposure it is in. Not saved (module doc).
#[derive(Debug, Clone, Default)]
pub struct Injuries {
    pub pending: Vec<Injury>,
    /// This exposure's heat so far, seconds at full intensity.
    dose_s: f64,
    /// Real seconds since heat last reached the body.
    since_heat_s: f64,
    /// The burn this exposure has given so far.
    burn: Option<String>,
}

/// Hurt `body`: the injury waits on it for the body pass. A body is anything the food system
/// keeps (Vitals and StatusEffects); the callers check.
pub fn injure(world: &mut hecs::World, body: hecs::Entity, injury: Injury) {
    if let Ok(mut inj) = world.get::<&mut Injuries>(body) {
        inj.pending.push(injury);
        return;
    }
    let _ = world.insert_one(body, Injuries { pending: vec![injury], ..Default::default() });
}

/// What the wounds did to a body this tick.
#[derive(Debug, Default)]
pub struct BodyTick {
    /// Health taken, each with the death screen's words for it.
    pub harm: Vec<(String, f64)>,
    /// What the player is told.
    pub notices: Vec<String>,
}

impl Wounds {
    /// Path of the file, relative to the data directory.
    pub const FILE: &'static str = "medical/wounds.ron";

    /// Disk first (modding), the copy built into the game when the disk copy is missing or this
    /// version cannot read it (BUG-163). With neither, no wounds and no healing: loud in the log,
    /// never a crash.
    pub fn load(data_dir: &Path) -> Self {
        crate::embedded_data::load_text_or_embedded(data_dir, Self::FILE, Self::from_ron).unwrap_or_else(|e| {
            log::warn!("{e}; nothing leaves a wound and nothing heals by itself until it is fixed");
            Self::default()
        })
    }

    /// Parse the file's text.
    pub fn from_ron(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    /// The row for a status effect, if it is a wound here.
    pub fn get(&self, effect: &str) -> Option<&Wound> {
        self.wounds.iter().find(|w| w.effect == effect)
    }

    /// The wounds on a body, in the order it got them.
    pub fn on<'a>(&'a self, effects: &'a StatusEffects) -> impl Iterator<Item = &'a Wound> + 'a {
        effects.active.iter().filter_map(move |e| self.get(&e.id))
    }

    /// This mode's share of a wound's course.
    pub fn course_share(&self, mode: Mode) -> f64 {
        match mode {
            Mode::Realistic => 1.0,
            Mode::Forgiving => f64::from(self.forgiving.course_share.clamp(0.0, 1.0)),
        }
    }

    /// This mode's share of a wound's harm.
    pub fn harm_share(&self, mode: Mode) -> f64 {
        match mode {
            Mode::Realistic => 1.0,
            Mode::Forgiving => f64::from(self.forgiving.harm_share.clamp(0.0, 1.0)),
        }
    }

    /// This mode's share of a wound's chance of going bad.
    pub fn goes_bad_share(&self, mode: Mode) -> f32 {
        match mode {
            Mode::Realistic => 1.0,
            Mode::Forgiving => self.forgiving.goes_bad_share.clamp(0.0, 1.0),
        }
    }

    /// A wound's whole course in this mode, game seconds: its Realistic course from
    /// status_effects.csv times this mode's share. 0 when the effect is not there.
    pub fn course_s(&self, effect: &str, registry: Option<&StatusEffectRegistry>, mode: Mode) -> f64 {
        registry.map_or(0.0, |r| f64::from(r.duration(effect))) * self.course_share(mode)
    }

    /// Give a body the wound `effect`, its whole course in this mode, and return what the player
    /// is told: its onset, how long it lasts and what helps. A wound that can go bad takes the
    /// care off (a new wound is not covered by it), and says so. None when `effect` has no row
    /// here or no course in status_effects.csv (logged).
    fn wound(&self, effects: &mut StatusEffects, effect: &str, registry: Option<&StatusEffectRegistry>, mode: Mode) -> Option<String> {
        let Some(w) = self.get(effect) else {
            log::warn!("[Wounds] {effect} has no row in {}; no wound given", Self::FILE);
            return None;
        };
        let course = self.course_s(effect, registry, mode);
        if course <= 0.0 {
            log::warn!("[Wounds] {effect} has no course in status_effects.csv; no wound given");
            return None;
        }
        effects.apply(effect, course);
        let mut told = format!("{} {} {}", w.onset, w.course.replace("{time}", &time_words(course)), w.helps);
        if w.goes_bad.is_some() && effects.has(&self.care.effect) {
            effects.remove(&self.care.effect);
            told.push(' ');
            told.push_str(&self.care.new_wound);
        }
        Some(told.trim().to_string())
    }

    /// Take the injuries waiting on a body (`injure`): each becomes its wounds, and heat takes
    /// health too. `real_dt` is this tick's real seconds, which a fire's exposure counts.
    pub fn take_injuries(
        &self,
        inj: &mut Injuries,
        effects: &mut StatusEffects,
        registry: Option<&StatusEffectRegistry>,
        mode: Mode,
        real_dt: f64,
        out: &mut BodyTick,
    ) {
        let mut heated = false;
        for injury in std::mem::take(&mut inj.pending) {
            match injury {
                Injury::Bite => {
                    let told: Vec<String> = self.bite.iter().filter_map(|e| self.wound(effects, e, registry, mode)).collect();
                    if !told.is_empty() {
                        out.notices.push(format!("You are bitten. {}", told.join(" ")));
                    }
                }
                Injury::Heat { dose_s } => {
                    let dose_s = dose_s.max(0.0);
                    if dose_s <= 0.0 {
                        continue;
                    }
                    heated = true;
                    if inj.since_heat_s > f64::from(self.burns.new_burn_after_s) {
                        // A new exposure: a new burn, not a deeper old one.
                        inj.dose_s = 0.0;
                        inj.burn = None;
                    }
                    inj.since_heat_s = 0.0;
                    inj.dose_s += dose_s;
                    out.harm.push((
                        self.burns.cause.clone(),
                        dose_s * f64::from(self.burns.health_per_s.max(0.0)) * self.harm_share(mode),
                    ));
                    let depth = self.burns.steps.iter().filter(|s| inj.dose_s >= f64::from(s.from_s)).last().map(|s| s.effect.clone());
                    if let Some(depth) = depth {
                        if inj.burn.as_deref() != Some(depth.as_str()) {
                            // Deeper: this exposure's shallower burn is this one now.
                            if let Some(old) = inj.burn.take() {
                                effects.remove(&old);
                            }
                            if let Some(told) = self.wound(effects, &depth, registry, mode) {
                                out.notices.push(told);
                            }
                            inj.burn = Some(depth);
                        }
                    }
                }
            }
        }
        if !heated {
            inj.since_heat_s += real_dt.max(0.0);
        }
    }

    /// The health the wounds on a body take in `game_s` game seconds, each with its cause. A
    /// wound that ends within them takes only its share up to the end: a night asleep moves the
    /// clock 720 game seconds a frame, longer than a whole bleed.
    pub fn harm(&self, effects: &StatusEffects, mode: Mode, game_s: f64, out: &mut BodyTick) {
        for active in &effects.active {
            let Some(w) = self.get(&active.id) else { continue };
            if w.health_per_h <= 0.0 {
                continue;
            }
            let secs = game_s.max(0.0).min(active.remaining.max(0.0));
            let amt = f64::from(w.health_per_h) / HOUR_S * secs * self.harm_share(mode);
            out.harm.push((w.cause.clone(), amt));
        }
    }

    /// Whether anything on a body keeps it from healing: a wound that stops healing, or an
    /// illness (a `disease` effect).
    pub fn healing_stopped(&self, effects: &StatusEffects, registry: Option<&StatusEffectRegistry>) -> bool {
        self.on(effects).any(|w| w.stops_healing)
            || effects.active.iter().any(|e| registry.and_then(|r| r.get(&e.id)).is_some_and(|d| d.is_disease()))
    }

    /// Health a body heals by itself in `game_s` game seconds: `max_health` over the mode's days,
    /// faster asleep, slower while Hungry or Thirsty, and none when `harmed` this tick (anything
    /// took health) or when healing is stopped (`healing_stopped`).
    pub fn healing(
        &self,
        effects: &StatusEffects,
        registry: Option<&StatusEffectRegistry>,
        mode: Mode,
        max_health: f32,
        asleep: bool,
        harmed: bool,
        game_s: f64,
    ) -> f64 {
        if harmed || game_s <= 0.0 || self.healing_stopped(effects, registry) {
            return 0.0;
        }
        let rate = self.healing_per_awake_s(mode, max_health);
        let asleep_factor = if asleep { f64::from(self.healing.asleep.max(0.0)) } else { 1.0 };
        let slowed = if self.healing.slowed_by.iter().any(|id| effects.has(id)) {
            f64::from(self.healing.slowed_share.clamp(0.0, 1.0))
        } else {
            1.0
        };
        rate * asleep_factor * slowed * game_s
    }

    /// Health healed in a waking game second, in this mode: `max_health` over the mode's days, a
    /// day being the waking hours and a night's sleep (`sleep::SLEEP_HOURS`) at the asleep rate.
    pub fn healing_per_awake_s(&self, mode: Mode, max_health: f32) -> f64 {
        let days = f64::from(match mode {
            Mode::Realistic => self.healing.realistic_days,
            Mode::Forgiving => self.healing.forgiving_days,
        });
        let asleep_h = crate::systems::sleep::SLEEP_HOURS;
        let awake_equivalent_h = (24.0 - asleep_h) + asleep_h * f64::from(self.healing.asleep.max(0.0));
        if days <= 0.0 || awake_equivalent_h <= 0.0 {
            return 0.0;
        }
        f64::from(max_health.max(0.0)) / (days * awake_equivalent_h * HOUR_S)
    }

    /// Wounds that go bad as the next `step_of(id)` game seconds pass: each that can, the tick
    /// its `after_h` comes, with the chance its care decides (`roll` gives a number in 0..1).
    /// What it becomes is given with its course, and the player is told.
    pub fn going_bad(
        &self,
        effects: &mut StatusEffects,
        registry: Option<&StatusEffectRegistry>,
        mode: Mode,
        step_of: impl Fn(&str) -> f64,
        mut roll: impl FnMut() -> f32,
        out: &mut BodyTick,
    ) {
        let cared = effects.has(&self.care.effect);
        let mut turned: Vec<(String, String)> = Vec::new();
        for active in &effects.active {
            let Some(w) = self.get(&active.id) else { continue };
            let Some(bad) = &w.goes_bad else { continue };
            let full = self.course_s(&active.id, registry, mode);
            let age_before = full - active.remaining;
            let age_after = age_before + step_of(&active.id).max(0.0);
            let when = f64::from(bad.after_h.max(0.0)) * HOUR_S;
            if !(age_before < when && when <= age_after) {
                continue;
            }
            let chance = if cared { bad.cared } else { bad.uncared } * self.goes_bad_share(mode);
            if roll() < chance {
                turned.push((active.id.clone(), bad.becomes.clone()));
            }
        }
        for (from, becomes) in turned {
            let name = registry.and_then(|r| r.get(&from)).map_or(from.as_str(), |d| d.name.as_str());
            log::info!("[Wounds] {from} went bad: {becomes}");
            if let Some(told) = self.wound(effects, &becomes, registry, mode) {
                out.notices.push(format!("Your {} has gone bad. {told}", name.to_lowercase()));
            }
        }
    }

    /// What the player is told about the wounds that heal as the next `step_of(id)` game seconds
    /// pass: each one's `healed` line.
    pub fn healed(&self, effects: &StatusEffects, step_of: impl Fn(&str) -> f64) -> Vec<String> {
        effects
            .active
            .iter()
            .filter(|e| e.remaining - step_of(&e.id) <= 0.0)
            .filter_map(|e| self.get(&e.id).map(|w| w.healed.clone()))
            .collect()
    }

    /// Whether a body has a wound that can go bad and nobody has cleaned and covered it.
    pub fn needs_care(&self, effects: &StatusEffects) -> bool {
        !self.care.effect.is_empty() && !effects.has(&self.care.effect) && self.longest_open(effects).is_some()
    }

    /// Clean and cover a body's wounds (a Bandage, a Medkit, Antiseptic): true when they
    /// `needs_care`, and then the care is on for as long as the longest wound that can go bad
    /// still has to heal.
    pub fn care_for(&self, effects: &mut StatusEffects) -> bool {
        if !self.needs_care(effects) {
            return false;
        }
        match self.longest_open(effects) {
            Some(left) => {
                effects.apply(&self.care.effect, left);
                true
            }
            None => false,
        }
    }

    /// Keep the care marker as long as the longest wound that can go bad, and take it off when
    /// none is left. Each tick, before the effects count down.
    pub fn keep_care(&self, effects: &mut StatusEffects) {
        if self.care.effect.is_empty() || !effects.has(&self.care.effect) {
            return;
        }
        match self.longest_open(effects) {
            Some(left) => {
                if let Some(c) = effects.active.iter_mut().find(|e| e.id == self.care.effect) {
                    c.remaining = left;
                }
            }
            None => effects.remove(&self.care.effect),
        }
    }

    /// Time left on the longest wound on a body that can go bad.
    fn longest_open(&self, effects: &StatusEffects) -> Option<f64> {
        effects
            .active
            .iter()
            .filter(|e| self.get(&e.id).is_some_and(|w| w.goes_bad.is_some()))
            .map(|e| e.remaining)
            .filter(|r| *r > 0.0)
            .reduce(f64::max)
    }

    /// Each wound on a body, for the HUD and the Inventory page: what it is, how long it still
    /// has, what helps, and for one that can go bad whether it is cared for.
    pub fn lines(&self, effects: &StatusEffects, registry: Option<&StatusEffectRegistry>) -> Vec<CareLine> {
        let cared = effects.has(&self.care.effect);
        effects
            .active
            .iter()
            .filter_map(|e| {
                let w = self.get(&e.id)?;
                let name = registry.and_then(|r| r.get(&e.id)).map_or(e.id.clone(), |d| d.name.clone());
                let mut line = format!("{} {}", w.course.replace("{time}", &time_words(e.remaining)), w.helps);
                let mut short: Vec<&str> = vec![w.hud.as_str()];
                if w.goes_bad.is_some() {
                    line.push(' ');
                    line.push_str(if cared { &self.care.cared } else { &self.care.uncared });
                    short.push(if cared { &self.care.cared_hud } else { &self.care.uncared_hud });
                }
                let short: Vec<&str> = short.into_iter().filter(|s| !s.is_empty()).collect();
                let severity = if w.goes_bad.is_some() && !cared { w.severity.max(1) } else { w.severity };
                Some(CareLine { kind: "Wound", name, short: short.join("; "), line, severity })
            })
            .collect()
    }
}

/// A wound or an illness on the player, for the HUD and the Inventory page.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CareLine {
    /// The HUD row's label: "Wound" or "Ill".
    pub kind: &'static str,
    /// Its name ("Bleeding").
    pub name: String,
    /// A few words for the HUD after the name ("a Bandage stops it").
    pub short: String,
    /// The Inventory page's line after the name: how it passes, what helps.
    pub line: String,
    /// 0 fine, 1 attention, 2 danger.
    pub severity: u8,
}

/// Every wound and illness on the player, for the HUD and the Inventory page (the lib.rs vitals
/// bridge copies them into `GuiVitals::care`), read with the data as shipped (disk first).
pub fn care_lines(effects: &StatusEffects, registry: Option<&StatusEffectRegistry>) -> Vec<CareLine> {
    let (wounds, illnesses, _) = shipped();
    let mut lines = wounds.lines(effects, registry);
    lines.extend(illness_lines(illnesses, effects, registry));
    lines
}

/// Each illness on a body, the way `Wounds::lines` gives each wound.
pub fn illness_lines(ill: &Illnesses, effects: &StatusEffects, registry: Option<&StatusEffectRegistry>) -> Vec<CareLine> {
    effects
        .active
        .iter()
        .filter_map(|e| {
            let i = ill.get(&e.id)?;
            let name = registry.and_then(|r| r.get(&e.id)).map_or(e.id.clone(), |d| d.name.clone());
            let line = format!("It passes on its own in about {}. {}", time_words(e.remaining), i.helps);
            Some(CareLine { kind: "Ill", name, short: i.hud.clone(), line, severity: 1 })
        })
        .collect()
}

/// The wound and illness data and the status effects as shipped (disk first, like the food
/// system's copies, BUG-163's rule for the status effects), for the GUI bridge and the Settings
/// hint, which have no FoodSystem to ask.
fn shipped() -> &'static (Wounds, Illnesses, Option<StatusEffectRegistry>) {
    static SHIPPED: std::sync::OnceLock<(Wounds, Illnesses, Option<StatusEffectRegistry>)> = std::sync::OnceLock::new();
    SHIPPED.get_or_init(|| {
        let dir = crate::data_dir();
        let effects = crate::embedded_data::load_data_or_embedded(&dir, "status_effects.csv", StatusEffectRegistry::from_csv)
            .map_err(|e| log::warn!("{e}; the injury hint has no courses to name"))
            .ok();
        (Wounds::load(&dir), Illnesses::load(&dir), effects)
    })
}

/// A length of time in plain words, minutes below an hour: "10 minutes", "a minute", and above
/// it `illness::duration_words` ("6 days").
pub fn time_words(s: f64) -> String {
    if s < 3600.0 {
        let m = (s / 60.0).round().max(1.0);
        if m <= 1.0 {
            "a minute".to_string()
        } else {
            format!("{m:.0} minutes")
        }
    } else {
        duration_words(s as f32)
    }
}

/// Settings > Gameplay > Illness and injury's second hint (2026-10-12): what the setting does to
/// wounds and healing, with the numbers read from the data it describes.
pub fn mode_hint() -> String {
    let (w, _, reg) = shipped();
    hint_text(w, reg.as_ref())
}

/// The hint's words.
fn hint_text(w: &Wounds, reg: Option<&StatusEffectRegistry>) -> String {
    if w.wounds.is_empty() {
        return "No wound data is loaded, so nothing leaves a wound and nothing heals by itself.".to_string();
    }
    let course = |id: &str| reg.map_or(0.0, |r| f64::from(r.duration(id)));
    let bleed = w.bite.first().map_or(0.0, |id| course(id));
    format!(
        "Injuries: a bite bleeds for about {} unless a Bandage or a Medkit stops it, and a burn \
         takes days or weeks to heal by its depth. A wound left uncleaned and uncovered can go \
         bad, and Antibiotics end an infected wound. A body that is fed, watered and unhurt heals \
         by itself, {:.1} times as fast asleep, and not while bleeding, infected or ill. \
         Realistic: from nearly dead to whole in about {:.0} days. Forgiving: about {:.0} days, \
         and wounds last {:.0}% as long, do {:.0}% of the harm and go bad {:.0}% as often.",
        time_words(bleed),
        w.healing.asleep,
        w.healing.realistic_days,
        w.healing.forgiving_days,
        w.forgiving.course_share * 100.0,
        w.forgiving.harm_share * 100.0,
        w.forgiving.goes_bad_share * 100.0,
    )
}

// The tests drive the food system, which applies all of this, so they are a child of `food`:
// src/systems/wounds_tests.rs.

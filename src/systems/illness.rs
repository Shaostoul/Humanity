//! Illness: what an illness does to the body's water, and what helps (BUG-162, 2026-10-05).
//!
//! The one illness the game gives anyone is the stomach illness from spoiled or raw food,
//! Food Poisoning (the food system applies it when such food is eaten, src/systems/food.rs).
//! Until 2026-10-05 it took 3 health every 15 s for 90 minutes, which emptied a full health
//! bar in about 8 minutes, and nothing removed it. A real stomach illness from food is mostly
//! the body losing water through vomiting and diarrhoea for a day or a few days, and then it
//! passes; most healthy adults get better with fluids and rest, and the danger is drying out
//! (the Library's When Food or Water Makes You Sick). So here an illness TAKES WATER from the
//! body's Hydration, on the game clock, for its course, and does no harm of its own: harm comes
//! only through dehydration, when the Hydration bar runs empty because the player did not
//! drink (food.rs). Drinking puts the water back, oral rehydration solution most of all
//! (`Illnesses::drink_kept`), and the course ends by itself.
//!
//! WHAT IS DATA. An illness is a status effect of type `disease` in data/status_effects.csv
//! (its name, its Realistic course `duration_s`, its tags) with a row in
//! data/medical/illnesses.ron (the water it takes a day, what the player is told). Which
//! medicines end an effect is decided by its tags (data/medical/treatments.ron,
//! `systems::treatment`): antibiotics end only effects tagged `bacterial`, and Food Poisoning
//! is not one.
//!
//! THE CLOCK. Every `disease` effect counts down, and does whatever it does, on the game
//! clock, with the body's other daily needs: the time-speed setting and a night asleep move
//! it along. The other effects count real seconds (food.rs, the note at the top of its tick).
//!
//! TWO MODES (the house rule for deep systems, CLAUDE.md "Dual modes"). Realistic runs the
//! course and takes the water the data gives. Forgiving, the default, runs
//! `forgiving.course_share` of the course and takes `forgiving.water_share` of the water: half
//! of each in the shipped data, the same "half" as body heat's Forgiving mode. Settings >
//! Gameplay > Illness, published to the DataStore under `MODE_KEY` by engine::survival_env.
//!
//! Pure data and math with no engine state, so it compiles in every feature set (the relay
//! build includes `systems/`).

use std::path::Path;

use serde::Deserialize;

use crate::ecs::components::StatusEffects;
use crate::hot_reload::data_store::DataStore;
use crate::systems::status_effects::StatusEffectRegistry;

/// DataStore key of the Settings mode (a plain `Mode`), published each frame by
/// `engine::survival_env` from Settings > Gameplay > Illness.
pub const MODE_KEY: &str = "illness_mode";

/// The `type` of status effect that is an illness: it runs on the game clock.
pub const DISEASE: &str = "disease";

/// Game seconds in a day, for the data's litres a day.
const DAY_S: f32 = crate::systems::time::EARTH_DAY_S as f32;

/// The two modes of the house rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// The default: a shorter, milder course (`Illnesses::forgiving`).
    #[default]
    Forgiving,
    /// The course and the water loss the data gives.
    Realistic,
}

impl Mode {
    /// The Settings mode. Absent (headless tests, the relay) is Realistic: the course the
    /// data gives, the same convention body heat uses.
    pub fn from_store(data: &DataStore) -> Self {
        data.get::<Mode>(MODE_KEY).copied().unwrap_or(Mode::Realistic)
    }

    /// The mode the saved setting names (`AppConfig::illness_realistic`).
    pub fn from_realistic(realistic: bool) -> Self {
        if realistic {
            Mode::Realistic
        } else {
            Mode::Forgiving
        }
    }
}

/// Forgiving's shares of the Realistic course and water loss.
#[derive(Debug, Clone, Deserialize)]
pub struct Shares {
    pub course_share: f32,
    pub water_share: f32,
}

/// One illness: a row of data/medical/illnesses.ron.
#[derive(Debug, Clone, Deserialize)]
pub struct Illness {
    /// The status effect it is (data/status_effects.csv, type `disease`).
    pub effect: String,
    /// Litres of the body's water it takes a day while it lasts, Realistic.
    pub water_l_per_day: f32,
    /// What the player is told when they fall ill.
    pub onset: String,
    /// What helps, told when they fall ill and when a medicine cannot help.
    pub helps: String,
    /// What the player is told when it has passed.
    pub passed: String,
}

/// How much of a drink's water a body keeps while an illness is taking water.
#[derive(Debug, Clone, Deserialize)]
pub struct DrinksWhileIll {
    /// The share kept for a drink not listed in `kept`.
    pub others: f32,
    /// (item id, share kept).
    #[serde(default)]
    pub kept: Vec<(String, f32)>,
}

/// data/medical/illnesses.ron.
#[derive(Debug, Clone, Deserialize)]
pub struct Illnesses {
    pub forgiving: Shares,
    #[serde(default)]
    pub illnesses: Vec<Illness>,
    pub drinks_while_ill: DrinksWhileIll,
}

impl Default for Illnesses {
    /// No illness data: nothing takes water, every drink keeps all of it, and both modes run
    /// the whole course.
    fn default() -> Self {
        Self {
            forgiving: Shares { course_share: 1.0, water_share: 1.0 },
            illnesses: Vec::new(),
            drinks_while_ill: DrinksWhileIll { others: 1.0, kept: Vec::new() },
        }
    }
}

impl Illnesses {
    /// Path of the file, relative to the data directory.
    pub const FILE: &'static str = "medical/illnesses.ron";

    /// Disk first (modding), the copy built into the game when the disk copy is missing or
    /// this version cannot read it (BUG-163, `embedded_data::load_text_or_embedded`). With
    /// neither there is no illness data, which means an illness takes no water: loud in the
    /// log, never a crash.
    pub fn load(data_dir: &Path) -> Self {
        crate::embedded_data::load_text_or_embedded(data_dir, Self::FILE, Self::from_ron).unwrap_or_else(|e| {
            log::warn!("{e}; illnesses take no water until it is fixed");
            Self::default()
        })
    }

    /// Parse the file's text.
    pub fn from_ron(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    /// The row for a status effect, if it is an illness here.
    pub fn get(&self, effect: &str) -> Option<&Illness> {
        self.illnesses.iter().find(|i| i.effect == effect)
    }

    /// The share of the Realistic course this mode runs.
    pub fn course_share(&self, mode: Mode) -> f32 {
        match mode {
            Mode::Realistic => 1.0,
            Mode::Forgiving => self.forgiving.course_share.clamp(0.0, 1.0),
        }
    }

    /// The share of the Realistic water loss this mode takes.
    pub fn water_share(&self, mode: Mode) -> f32 {
        match mode {
            Mode::Realistic => 1.0,
            Mode::Forgiving => self.forgiving.water_share.clamp(0.0, 1.0),
        }
    }

    /// The illnesses on a body, in the order it caught them.
    pub fn on<'a>(&'a self, effects: &'a StatusEffects) -> impl Iterator<Item = &'a Illness> + 'a {
        effects.active.iter().filter_map(move |e| self.get(&e.id))
    }

    /// Litres of water the illnesses on a body take in `game_dt` game seconds, in `mode`.
    pub fn water_l(&self, effects: &StatusEffects, mode: Mode, game_dt: f32) -> f32 {
        let per_day: f32 = self.on(effects).map(|i| i.water_l_per_day.max(0.0)).sum();
        per_day * self.water_share(mode) * game_dt.max(0.0) / DAY_S
    }

    /// The share of a drink's water a body keeps: all of it while well, and while an illness
    /// is taking water, the share `drinks_while_ill` gives the drink.
    pub fn drink_kept(&self, effects: &StatusEffects, item_id: &str) -> f32 {
        if !self.on(effects).any(|i| i.water_l_per_day > 0.0) {
            return 1.0;
        }
        let d = &self.drinks_while_ill;
        d.kept.iter().find(|(id, _)| id == item_id).map_or(d.others, |(_, share)| *share).clamp(0.0, 1.0)
    }

    /// What the player is told when they fall ill: the data's onset line, how long it lasts
    /// (`course_s` game seconds, in this mode) and what helps.
    pub fn onset_notice(illness: &Illness, course_s: f32) -> String {
        format!("{} It passes on its own in about {}. {}", illness.onset, duration_words(course_s), illness.helps)
    }

    /// What helps with each illness on a body, named, for a medicine that cannot help:
    /// "For Food Poisoning: Drink while it lasts. ..." One line per illness.
    pub fn helps_lines(&self, effects: &StatusEffects, registry: Option<&StatusEffectRegistry>) -> Vec<String> {
        self.on(effects)
            .map(|i| {
                let name = registry.and_then(|r| r.get(&i.effect)).map_or(i.effect.as_str(), |d| d.name.as_str());
                format!("For {name}: {}", i.helps)
            })
            .collect()
    }

    /// The name of the first illness on a body that takes water, for a death from dehydration
    /// while it lasted.
    pub fn drying_illness<'a>(
        &self,
        effects: &StatusEffects,
        registry: Option<&'a StatusEffectRegistry>,
    ) -> Option<String> {
        self.on(effects).find(|i| i.water_l_per_day > 0.0).map(|i| {
            registry.and_then(|r| r.get(&i.effect)).map_or_else(|| i.effect.clone(), |d| d.name.clone())
        })
    }
}

/// A length of time in plain words: "2 days", "a day", "18 hours", "an hour".
pub fn duration_words(s: f32) -> String {
    let hours = (s / 3600.0).round().max(1.0);
    if hours >= 36.0 {
        format!("{:.0} days", (hours / 24.0).round())
    } else if hours >= 20.0 {
        "a day".to_string()
    } else if hours >= 2.0 {
        format!("{hours:.0} hours")
    } else {
        "an hour".to_string()
    }
}

/// The illness data as shipped (disk first, like the food system's copy), for the Settings
/// hint, which has no DataStore.
fn shipped() -> &'static (Illnesses, Option<StatusEffectRegistry>) {
    static SHIPPED: std::sync::OnceLock<(Illnesses, Option<StatusEffectRegistry>)> = std::sync::OnceLock::new();
    SHIPPED.get_or_init(|| hint_data(&crate::data_dir()))
}

/// The illnesses and the status effects as the game loads them, by the rule every registry
/// loads by (BUG-163): a data folder's status_effects.csv this version cannot read gives the
/// built-in copy, not an empty table, so the hint never says "about an hour" for want of the
/// illness's course.
fn hint_data(dir: &Path) -> (Illnesses, Option<StatusEffectRegistry>) {
    let effects = crate::embedded_data::load_data_or_embedded(dir, "status_effects.csv", StatusEffectRegistry::from_csv)
        .map_err(|e| log::warn!("{e}; the illness hint has no course to name"))
        .ok();
    (Illnesses::load(dir), effects)
}

/// Settings > Gameplay > Illness's hint (2026-10-05): what each mode does, with the numbers
/// read from the data it describes, so the two cannot drift.
pub fn mode_hint() -> String {
    hint_text(shipped())
}

/// The hint's words, from the data `hint_data` loaded.
fn hint_text((ill, effects): &(Illnesses, Option<StatusEffectRegistry>)) -> String {
    let Some(i) = ill.illnesses.first() else {
        return "No illness data is loaded, so illnesses take no water in either mode.".to_string();
    };
    let def = effects.as_ref().and_then(|r| r.get(&i.effect));
    let name = def.map_or(i.effect.as_str(), |d| d.name.as_str());
    let course = def.map_or(0.0, |d| d.duration_s);
    let (cs, ws) = (ill.course_share(Mode::Forgiving), ill.water_share(Mode::Forgiving));
    // What a resting body loses a day anyway, from the food system's own clock.
    use crate::systems::food::{HYDRATION_DECAY_PER_SEC, HYDRATION_PER_LITRE};
    let daily_l = HYDRATION_DECAY_PER_SEC * DAY_S / HYDRATION_PER_LITRE;
    format!(
        "Illness from spoiled or raw food ({name}). Vomiting and diarrhoea take water out of you \
         until it passes on its own: drink while it lasts and it does no harm, go without and \
         you dry out. Realistic: it lasts about {} and takes {:.1} L of water a day on top of \
         the {daily_l:.1} L a body needs anyway. Forgiving: about {} and {:.2} L a day. Oral \
         rehydration solution puts back the most; antibiotics do not help it.",
        duration_words(course),
        i.water_l_per_day,
        duration_words(course * cs),
        i.water_l_per_day * ws,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped_data() -> Illnesses {
        Illnesses::from_ron(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/data/medical/illnesses.ron")).unwrap())
            .expect("illnesses.ron parses")
    }

    /// The shipped file parses, every illness in it is a `disease` row of
    /// status_effects.csv, and Forgiving is milder by both shares.
    #[test]
    fn shipped_illnesses_parse_and_name_disease_effects() {
        let ill = shipped_data();
        let reg = StatusEffectRegistry::from_csv(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/status_effects.csv")))
            .expect("status_effects.csv");
        assert!(!ill.illnesses.is_empty());
        for i in &ill.illnesses {
            let def = reg.get(&i.effect).unwrap_or_else(|| panic!("{} is not a status effect", i.effect));
            assert_eq!(def.kind, DISEASE, "{} is an illness, so it is a disease row", i.effect);
            assert!(def.duration_s > 0.0, "{} passes: it has a course", i.effect);
            assert_eq!(def.damage_per_tick, 0.0, "{} does no harm of its own: it takes water", i.effect);
            assert!(i.water_l_per_day > 0.0 && !i.onset.is_empty() && !i.helps.is_empty() && !i.passed.is_empty());
        }
        assert!(ill.course_share(Mode::Forgiving) < 1.0 && ill.water_share(Mode::Forgiving) < 1.0, "Forgiving is milder");
        assert_eq!(ill.course_share(Mode::Realistic), 1.0);
    }

    /// A drink keeps all of its water while well, and the listed share while ill.
    #[test]
    fn a_drink_keeps_all_its_water_while_well() {
        let ill = shipped_data();
        let mut fx = StatusEffects::default();
        assert_eq!(ill.drink_kept(&fx, "water_bottle_0"), 1.0);
        fx.apply("food_poisoning", 60.0);
        assert_eq!(ill.drink_kept(&fx, "ors_solution_0"), 1.0);
        assert!(ill.drink_kept(&fx, "water_bottle_0") < 1.0);
    }

    /// The Settings hint states the shipped numbers: the two courses, the water each takes,
    /// and what a body loses anyway (the food system's own clock, 2.5 L a day).
    #[test]
    fn the_settings_hint_reads_its_numbers_from_the_data() {
        let hint = mode_hint();
        for part in ["Food Poisoning", "about 2 days", "1.5 L", "about a day", "0.75 L", "2.5 L"] {
            assert!(hint.contains(part), "the hint names {part:?}: {hint}");
        }
    }

    /// BUG-163: the hint loads its data the way the game does, so a data folder whose
    /// status_effects.csv this version cannot read (one written before 2026-10-05 still has
    /// the `dispel_type` column) gives the built-in course, never "about an hour" for want of
    /// one.
    ///
    /// Seen red with the old read (the folder's file used whatever was in it):
    ///   the hint names the course: Illness from spoiled or raw food (food_poisoning). ...
    ///   Realistic: it lasts about an hour ... Forgiving: about an hour and 0.75 L a day. ...
    #[test]
    fn the_settings_hint_names_the_course_with_a_data_folder_older_than_the_game() {
        let dir = crate::test_temp::dir("illness_hint");
        let stale = crate::systems::status_effects::with_old_dispel_type_column(crate::embedded_data::STATUS_EFFECTS_CSV);
        std::fs::write(dir.join("status_effects.csv"), stale).expect("write the file");
        let hint = hint_text(&hint_data(&dir));
        assert!(hint.contains("about 2 days"), "the hint names the course: {hint}");
    }

    #[test]
    fn durations_read_in_plain_words() {
        assert_eq!(duration_words(172_800.0), "2 days");
        assert_eq!(duration_words(86_400.0), "a day");
        assert_eq!(duration_words(6.0 * 3600.0), "6 hours");
        assert_eq!(duration_words(1800.0), "an hour");
    }
}

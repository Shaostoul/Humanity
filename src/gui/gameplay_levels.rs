//! THE GAMEPLAY SETTINGS' LEVELS (2026-11-11). The operator, that day: "I feel like we need to
//! provide more than two options for our gameplay settings. Including just turning settings off
//! completely. Like body heat, illness, etc."
//!
//! Every gameplay setting with levels (body heat, illness and injury, farm animals, carrying
//! weight, death, the mining drone, the walking pace, animation, hostile wildlife, the vitals
//! drain) is a row of `data/gameplay/levels.json`: its levels in the order Settings shows them,
//! each with its name and a sentence saying what it changes (keys in data/i18n/en.json), and the
//! level a new player gets. Settings > Gameplay draws the file; nothing here lists a setting.
//!
//! SAVED as `AppConfig::gameplay`, setting id to level id. A setting never chosen, a level the
//! file no longer lists, and a config.json from before the levels (when each was a true/false
//! switch) all read as the file's default: no backwards compatibility before v1.0.
//!
//! WHAT A LEVEL MEANS is its system's: each system turns the id into its own mode
//! (`body_heat::Mode::from_level` and its kin), and [`understood`] asks each of them, so a level
//! added to the file that its system does not know fails `every_level_is_understood_by_its_system`
//! instead of silently playing as the default. Two settings use numbers: hostile wildlife's
//! `value` is the share of each wild pack that spawns, and the vitals drain's levels are points on
//! its slider. The walking pace's id names its rows in data/game.csv (`walk_speed_<id>_mps`).
//!
//! READ ONCE for the run (the data folder's copy, else the exe's own), like the warning bar's
//! mute choices: a setting's menu, not something that goes stale.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

/// The file, under data/.
pub const REL: &str = "gameplay/levels.json";

/// One level of a setting.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Level {
    /// What config.json saves and the system reads.
    pub id: String,
    /// Its name's key.
    pub words: String,
    /// The key of the sentence saying what it changes.
    pub says: String,
    /// A number the setting uses as it is (the share of a pack, the drain's rate).
    #[serde(default)]
    pub value: Option<f32>,
}

/// One setting and its levels, gentlest first.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Setting {
    pub id: String,
    /// Its name's key.
    pub words: String,
    /// A longer note under the choices, by key, for what holds at every level (optional).
    #[serde(default)]
    pub note: Option<String>,
    /// The level a new player gets.
    pub default: String,
    /// A slider's range beside the levels, for a setting whose levels are numbers.
    #[serde(default)]
    pub slider: Option<[f32; 2]>,
    pub levels: Vec<Level>,
}

impl Setting {
    /// The level `id`, if this setting has it.
    pub fn level(&self, id: &str) -> Option<&Level> {
        self.levels.iter().find(|l| l.id == id)
    }
}

/// The file.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LevelsFile {
    #[serde(rename = "_doc", default)]
    _doc: Vec<String>,
    pub settings: Vec<Setting>,
}

impl LevelsFile {
    /// Read the file, held to its rules: unique ids, at least two levels each (more than two
    /// wherever a system can be off), a default that is one of its levels, and a number on every
    /// level of a setting that uses numbers.
    pub fn parse(text: &str) -> Result<Self, String> {
        let file: LevelsFile = serde_json::from_str(text).map_err(|e| format!("data/{REL}: {e}"))?;
        let mut seen = std::collections::HashSet::new();
        for s in &file.settings {
            if !seen.insert(s.id.as_str()) {
                return Err(format!("data/{REL}: the setting `{}` is listed twice", s.id));
            }
            if s.levels.len() < 2 {
                return Err(format!("data/{REL}: the setting `{}` has {} level(s); a choice needs two or more", s.id, s.levels.len()));
            }
            if s.level("off").is_some() && s.levels.len() < 3 {
                return Err(format!("data/{REL}: the setting `{}` has Off and only one other level; give it more than two", s.id));
            }
            let mut ids = std::collections::HashSet::new();
            for l in &s.levels {
                if !ids.insert(l.id.as_str()) {
                    return Err(format!("data/{REL}: the setting `{}` lists the level `{}` twice", s.id, l.id));
                }
            }
            if s.level(&s.default).is_none() {
                return Err(format!("data/{REL}: the setting `{}`'s default `{}` is not one of its levels", s.id, s.default));
            }
            let numbers = s.levels.iter().filter(|l| l.value.is_some()).count();
            if numbers != 0 && numbers != s.levels.len() {
                return Err(format!("data/{REL}: the setting `{}` gives a number to some levels and not others", s.id));
            }
            if s.slider.is_some() && numbers == 0 {
                return Err(format!("data/{REL}: the setting `{}` has a slider but its levels are not numbers", s.id));
            }
        }
        Ok(file)
    }
}

/// The file, read once for the run. Empty only when both the data folder's copy and the exe's
/// own fail to load (a lib test rules out the second): then every setting reads its fallback.
pub fn file() -> &'static LevelsFile {
    static FILE: OnceLock<LevelsFile> = OnceLock::new();
    FILE.get_or_init(|| {
        humanity_core::embedded_data::load_text_or_embedded(&humanity_core::storage::data_dir(), REL, LevelsFile::parse).unwrap_or_else(|e| {
            log::warn!("{e}: the gameplay settings play at their fallbacks this run");
            LevelsFile::default()
        })
    })
}

/// The setting `id`.
pub fn setting(id: &str) -> Option<&'static Setting> {
    file().settings.iter().find(|s| s.id == id)
}

/// The level `saved` holds for the setting `id`, or its default when it holds none the file
/// lists. "" only for a setting the file does not have.
pub fn level_of<'a>(saved: &'a BTreeMap<String, String>, id: &str) -> &'a str {
    let Some(s) = setting(id) else { return "" };
    match saved.get(id) {
        Some(l) if s.level(l).is_some() => l.as_str(),
        _ => s.default.as_str(),
    }
}

/// The number of the level `saved` holds for `id` (hostile wildlife's share of a pack), 0 when
/// the setting or its number is missing.
pub fn value_of(saved: &BTreeMap<String, String>, id: &str) -> f32 {
    setting(id).and_then(|s| s.level(level_of(saved, id))).and_then(|l| l.value).unwrap_or(0.0)
}

/// Does the system behind `setting` know the level `level`? The one place a setting's id meets
/// its system; every level in the file must answer true (the test below).
pub fn understood(setting: &str, level: &str) -> bool {
    use humanity_systems as sys;
    match setting {
        "body_heat" => sys::body_heat::Mode::from_level(level).is_some(),
        "illness" => sys::illness::Mode::from_level(level).is_some(),
        "animal_care" => sys::animal_care::Mode::from_level(level).is_some(),
        "carry" => sys::encumbrance::CarryMode::from_level(level).is_some(),
        "death" => sys::death_pack::DeathMode::from_level(level).is_some(),
        "mining_drone" => sys::mining::Mode::from_level(level).is_some(),
        "animation" => matches!(level, "realistic" | "simplified"),
        // Its id names data/game.csv's rows: any id is read, and the test checks the rows exist.
        "walk_pace" => !level.is_empty(),
        // Numbers, used as they are.
        "hostile_wildlife" | "vitals_drain" => setting_has_value(setting, level),
        _ => false,
    }
}

fn setting_has_value(setting_id: &str, level: &str) -> bool {
    setting(setting_id).and_then(|s| s.level(level)).is_some_and(|l| l.value.is_some_and(|v| v.is_finite() && v >= 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> LevelsFile {
        let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data").join(REL);
        LevelsFile::parse(&std::fs::read_to_string(&path).expect("data/gameplay/levels.json")).expect("the shipped levels file reads")
    }

    /// THE LEVELS FILE LOADS, EVERY LEVEL HAS ITS WORDS, AND MORE THAN TWO WHERE A SYSTEM CAN BE
    /// OFF (2026-11-11): the shipped file reads (and the exe's own copy is the same file), each
    /// setting the brief names is in it, every name and every "what it changes" sentence is a key
    /// with words in en.json, and each of body heat, illness, farm animals, carrying weight,
    /// death, the mining drone, hostile wildlife and the vitals drain has an Off and at least two
    /// other levels.
    ///
    /// Seen red 2026-11-11 with body heat's Off pointing at a key en.json does not have:
    /// "body_heat/off: the key gameplay.body_heat_off_missing has no words".
    #[test]
    fn the_levels_file_loads_and_every_level_has_its_words() {
        let f = shipped();
        assert_eq!(LevelsFile::parse(humanity_core::embedded_data::GAMEPLAY_LEVELS_JSON).as_ref(), Ok(&f), "the exe's copy is the data folder's");
        let en: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data/i18n/en.json")).unwrap()).unwrap();
        let words = |key: &str| key.split('.').try_fold(&en, |v, k| v.get(k)).and_then(|v| v.as_str()).map(str::to_string);
        let mut missing = Vec::new();
        for s in &f.settings {
            for key in std::iter::once(&s.words).chain(s.note.iter()) {
                if words(key).is_none_or(|w| w.trim().is_empty()) {
                    missing.push(format!("{}: the key {key} has no words", s.id));
                }
            }
            for l in &s.levels {
                for key in [&l.words, &l.says] {
                    if words(key).is_none_or(|w| w.trim().is_empty()) {
                        missing.push(format!("{}/{}: the key {key} has no words", s.id, l.id));
                    }
                }
            }
        }
        assert!(missing.is_empty(), "{missing:#?}");
        for id in ["body_heat", "illness", "animal_care", "carry", "death", "mining_drone", "hostile_wildlife", "vitals_drain"] {
            let s = f.settings.iter().find(|s| s.id == id).unwrap_or_else(|| panic!("the setting {id} is in the file"));
            assert!(s.level("off").is_some() && s.levels.len() >= 3, "{id} has Off and at least two other levels: {:?}", s.levels.iter().map(|l| &l.id).collect::<Vec<_>>());
        }
        for id in ["walk_pace", "animation"] {
            assert!(f.settings.iter().any(|s| s.id == id), "the setting {id} is in the file");
        }
    }

    /// EVERY LEVEL IS UNDERSTOOD BY ITS SYSTEM: a level the file offers that the system does not
    /// know would play as the default while Settings showed it chosen. And every walking pace has
    /// its three rows in data/game.csv.
    ///
    /// Seen red 2026-11-11 with `CarryMode::from_level` not knowing "off": "carry/off is not
    /// understood".
    #[test]
    fn every_level_is_understood_by_its_system() {
        let f = shipped();
        let csv = std::fs::read_to_string(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data/game.csv")).unwrap();
        let mut wrong = Vec::new();
        for s in &f.settings {
            for l in &s.levels {
                if !understood(&s.id, &l.id) {
                    wrong.push(format!("{}/{} is not understood", s.id, l.id));
                }
                if s.id == "walk_pace" {
                    for row in ["walk_speed", "run_speed", "crouch_speed"] {
                        let key = format!("{row}_{}_mps,", l.id);
                        if !csv.lines().any(|line| line.starts_with(&key)) {
                            wrong.push(format!("walk_pace/{}: data/game.csv has no {key}", l.id));
                        }
                    }
                }
            }
        }
        assert!(wrong.is_empty(), "{wrong:#?}");
    }

    /// A SAVED LEVEL THE FILE DOES NOT LIST READS AS THE DEFAULT, and so does a setting never
    /// chosen (a config.json from before the levels has none).
    #[test]
    fn an_unknown_or_missing_level_reads_as_the_default() {
        let mut saved = BTreeMap::new();
        assert_eq!(level_of(&saved, "body_heat"), "forgiving", "never chosen: the default");
        saved.insert("body_heat".to_string(), "true".to_string());
        assert_eq!(level_of(&saved, "body_heat"), "forgiving", "an old switch's value: the default");
        saved.insert("body_heat".to_string(), "off".to_string());
        assert_eq!(level_of(&saved, "body_heat"), "off");
        assert_eq!(value_of(&saved, "hostile_wildlife"), 0.0, "hostile wildlife is off by default");
        saved.insert("hostile_wildlife".to_string(), "normal".to_string());
        assert_eq!(value_of(&saved, "hostile_wildlife"), 1.0);
    }

    /// The file's rules refuse what they say they refuse.
    #[test]
    fn the_rules_refuse_a_bad_file() {
        let bad = |text: &str, says: &str| {
            let e = LevelsFile::parse(text).expect_err(says);
            assert!(e.contains(says), "{e}");
        };
        let lv = |id: &str| format!(r#"{{ "id": "{id}", "words": "w", "says": "s" }}"#);
        bad(&format!(r#"{{ "settings": [ {{ "id": "a", "words": "w", "default": "on", "levels": [ {} ] }} ] }}"#, lv("on")), "a choice needs two or more");
        bad(&format!(r#"{{ "settings": [ {{ "id": "a", "words": "w", "default": "on", "levels": [ {}, {} ] }} ] }}"#, lv("off"), lv("on")), "give it more than two");
        bad(&format!(r#"{{ "settings": [ {{ "id": "a", "words": "w", "default": "x", "levels": [ {}, {} ] }} ] }}"#, lv("a"), lv("b")), "is not one of its levels");
    }
}

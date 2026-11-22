//! A SERVER'S GAMEPLAY RULES (2026-11-21; docs/design/server-rules.md). The operator, that day:
//! "Let's make sure the first server we have setup, via united-humanity is the forgiving game
//! mode. I think realistic will be too much for most people."
//!
//! The realism levels of Settings > Gameplay (data/gameplay/levels.json: body heat, illness,
//! farm animals, carrying weight, death, the drone, the walking pace, the garden's and the ship's
//! levels, skill learning, cooking...) are each person's own choice in solo play. In a server's
//! shared world they are the SERVER'S, so everyone there plays by the same rules (fairness at
//! MMO scale). This is that ruleset: a preset from data/gameplay/presets.json (Forgiving, the
//! default for a new server, or Realistic) or Custom, the admin's own level for each setting.
//!
//! STORED in the `server_settings` row (two columns, `gameplay_preset` and `gameplay_levels`,
//! the second a JSON object kept only for Custom), carried in `ServerSettings::gameplay_rules`
//! to Server Settings, set by an admin with `server_settings_update` (handlers/gameplay_rules.rs
//! tells every game at once), and sent to a game in its `game_welcome` as [`GameplayRules::wire`]:
//! the preset and the levels in play. A setting the levels in play do not name plays at its
//! default, the level a new player gets (gui/gameplay_levels.rs `level_of`).
//!
//! The relay holds the ruleset's SHAPE to the file (a preset it lists, or Custom with at most
//! [`MAX_LEVELS`] ids that look like ids); what each level means is the game's, which reads a
//! level its file does not list as the setting's default, as it does for a person's own choice.
//! Compiled in both builds (the relay and the desktop app, which includes the relay).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// The ruleset whose levels the admin picks one by one.
pub const CUSTOM: &str = "custom";

/// The presets' file, under data/.
pub const PRESETS_REL: &str = "gameplay/presets.json";

/// The exe's own copy, for a data folder without the file (the relay's VPS keeps data/ beside it,
/// but a server built without it still offers the presets).
const PRESETS_JSON: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/gameplay/presets.json"));

/// The most levels a Custom ruleset keeps (the levels file has 17 settings): a bound, so a
/// message cannot grow the row without limit.
pub const MAX_LEVELS: usize = 64;

/// A server's gameplay rules, as stored, as Server Settings shows them, and as a game is sent
/// them (where `levels` is [`GameplayRules::levels_in_play`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameplayRules {
    /// A preset's id from data/gameplay/presets.json, or [`CUSTOM`].
    pub preset: String,
    /// Setting id to level id. Stored only for Custom; sent to a game as the levels in play.
    #[serde(default)]
    pub levels: BTreeMap<String, String>,
}

impl Default for GameplayRules {
    /// A new server's: the file's default preset (Forgiving).
    fn default() -> Self {
        Self { preset: presets().default.clone(), levels: BTreeMap::new() }
    }
}

/// One preset of the file.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    pub id: String,
    /// Its name's key in data/i18n/en.json.
    pub words: String,
    /// The key of the sentence saying what it changes.
    pub says: String,
    /// Setting id to level id; a setting not named plays at its default.
    #[serde(default)]
    pub levels: BTreeMap<String, String>,
}

/// data/gameplay/presets.json.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresetsFile {
    #[serde(rename = "_doc", default)]
    _doc: Vec<String>,
    /// The preset a new server gets.
    pub default: String,
    pub presets: Vec<Preset>,
}

impl PresetsFile {
    /// Read the file, held to its rules: at least one preset, unique ids, none called "custom"
    /// (that is the admin's own ruleset), and a default that is one of them.
    pub fn parse(text: &str) -> Result<Self, String> {
        let file: PresetsFile = serde_json::from_str(text).map_err(|e| format!("data/{PRESETS_REL}: {e}"))?;
        let mut seen = std::collections::HashSet::new();
        for p in &file.presets {
            if p.id == CUSTOM {
                return Err(format!("data/{PRESETS_REL}: no preset may be called `{CUSTOM}`: that is the admin's own ruleset"));
            }
            if !seen.insert(p.id.as_str()) {
                return Err(format!("data/{PRESETS_REL}: the preset `{}` is listed twice", p.id));
            }
        }
        if !file.presets.iter().any(|p| p.id == file.default) {
            return Err(format!("data/{PRESETS_REL}: the default `{}` is not one of its presets", file.default));
        }
        Ok(file)
    }
}

/// The presets, read once for the run: the data folder's copy, else the exe's own (a menu, not
/// something that goes stale, like gui/gameplay_levels.rs's file).
pub fn presets() -> &'static PresetsFile {
    static FILE: OnceLock<PresetsFile> = OnceLock::new();
    FILE.get_or_init(|| {
        let path = humanity_core::storage::data_dir().join(PRESETS_REL);
        let from_disk = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display())).and_then(|t| PresetsFile::parse(&t));
        from_disk.unwrap_or_else(|e| {
            // Said in the log's built-in-copy words, so a rig refuses a run that served it (BUG-133).
            humanity_core::embedded_data::note_builtin_copy("gameplay/presets.json", format_args!("{e}: the gameplay rules' presets"));
            PresetsFile::parse(PRESETS_JSON).expect("the built-in presets file reads (a lib test checks it)")
        })
    })
}

/// The preset `id`.
pub fn preset(id: &str) -> Option<&'static Preset> {
    presets().presets.iter().find(|p| p.id == id)
}

/// Does `s` look like a setting's or a level's id (lowercase letters, digits and `_`, 1 to 40
/// characters)?
fn is_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 40 && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

impl GameplayRules {
    /// A preset of the file.
    pub fn preset(id: &str) -> Option<Self> {
        preset(id).map(|p| Self { preset: p.id.clone(), levels: BTreeMap::new() })
    }

    /// Custom, with these levels.
    pub fn custom(levels: BTreeMap<String, String>) -> Self {
        Self { preset: CUSTOM.to_string(), levels }
    }

    /// An admin's ruleset, held to its shape: a preset the file lists (its own levels rule, so
    /// any sent with it are dropped) or Custom, keeping at most [`MAX_LEVELS`] levels whose ids
    /// look like ids. None for anything else: a bad message changes nothing.
    pub fn checked(self) -> Option<Self> {
        if self.preset == CUSTOM {
            let levels: BTreeMap<String, String> = self.levels.into_iter().filter(|(s, l)| is_id(s) && is_id(l)).take(MAX_LEVELS).collect();
            return Some(Self::custom(levels));
        }
        Self::preset(&self.preset)
    }

    /// The levels in play: a preset's own, or Custom's. A setting not named plays at its default.
    pub fn levels_in_play(&self) -> BTreeMap<String, String> {
        match preset(&self.preset) {
            Some(p) => p.levels.clone(),
            None => self.levels.clone(),
        }
    }

    /// What a game in the shared world is sent: the preset and the levels in play.
    pub fn wire(&self) -> serde_json::Value {
        serde_json::json!({ "preset": self.preset, "levels": self.levels_in_play() })
    }

    /// A ruleset from a game message's `rules` (a welcome's, a `game_rules`'): None when absent
    /// or not a ruleset.
    pub fn from_wire(v: &serde_json::Value) -> Option<Self> {
        serde_json::from_value::<Self>(v.clone()).ok()
    }

    /// From the row's two columns. A preset the file no longer lists reads as the default
    /// preset, and Custom's levels as stored (an unreadable object as none).
    pub fn from_columns(preset: &str, levels: &str) -> Self {
        if preset == CUSTOM {
            let levels = serde_json::from_str::<BTreeMap<String, String>>(levels).unwrap_or_default();
            return Self::custom(levels).checked().unwrap_or_default();
        }
        Self::preset(preset).unwrap_or_default()
    }

    /// The `gameplay_levels` column: Custom's levels as a JSON object, "{}" for a preset.
    pub fn levels_column(&self) -> String {
        if self.preset == CUSTOM {
            serde_json::to_string(&self.levels).unwrap_or_else(|_| "{}".to_string())
        } else {
            "{}".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> PresetsFile {
        let path = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data").join(PRESETS_REL);
        PresetsFile::parse(&std::fs::read_to_string(&path).expect("data/gameplay/presets.json")).expect("the shipped presets file reads")
    }

    /// THE PRESETS FILE READS, AND A NEW SERVER IS FORGIVING (2026-11-21): the shipped file and
    /// the exe's copy are the same and read; its default is Forgiving, which names no level (every
    /// setting at the level a new player gets); Realistic and Forgiving are both in it.
    ///
    /// Seen red 2026-11-21 with the file's `"default": "realistic"`: "a new server plays Forgiving
    /// (the operator, 2026-11-21) / left: \"realistic\" / right: \"forgiving\"".
    #[test]
    fn the_presets_file_reads_and_a_new_server_is_forgiving() {
        let f = shipped();
        assert_eq!(PresetsFile::parse(PRESETS_JSON).as_ref(), Ok(&f), "the exe's copy is the data folder's");
        assert_eq!(f.default, "forgiving", "a new server plays Forgiving (the operator, 2026-11-21)");
        assert!(f.presets.iter().any(|p| p.id == "realistic"), "Realistic is offered");
        let rules = GameplayRules::default();
        assert_eq!(rules.preset, "forgiving");
        assert!(rules.levels_in_play().is_empty(), "Forgiving names no level: every setting plays at its default");
    }

    /// AN ADMIN'S RULESET IS HELD TO ITS SHAPE: a preset keeps none of the levels sent with it,
    /// Custom keeps only ids that look like ids, at most MAX_LEVELS of them, and a preset the file
    /// does not list changes nothing.
    ///
    /// Seen red 2026-11-21 with `checked` keeping every level (no `take(MAX_LEVELS)`): "left: 200 /
    /// right: 64".
    #[test]
    fn an_admins_ruleset_is_held_to_its_shape() {
        let mut lv = BTreeMap::new();
        lv.insert("body_heat".to_string(), "off".to_string());
        lv.insert("Body Heat!".to_string(), "off".to_string());
        lv.insert("illness".to_string(), "x".repeat(41));
        let sent = GameplayRules { preset: "realistic".into(), levels: lv.clone() };
        assert_eq!(sent.checked().map(|r| r.levels), Some(BTreeMap::new()), "a preset's own levels rule");
        let custom = GameplayRules::custom(lv).checked().expect("Custom is a ruleset");
        assert_eq!(custom.levels.len(), 1, "only the ids that look like ids: {:?}", custom.levels);
        let many: BTreeMap<String, String> = (0..200).map(|i| (format!("s{i}"), "on".to_string())).collect();
        assert_eq!(GameplayRules::custom(many).checked().unwrap().levels.len(), MAX_LEVELS);
        assert_eq!(GameplayRules { preset: "nightmare".into(), levels: BTreeMap::new() }.checked(), None, "not a preset: nothing changes");
        // The columns: a preset stores "{}", Custom its levels, and a preset the file no longer
        // lists reads as the default.
        assert_eq!(GameplayRules::default().levels_column(), "{}");
        assert_eq!(GameplayRules::from_columns("custom", &custom.levels_column()), custom);
        assert_eq!(GameplayRules::from_columns("gone", "{}"), GameplayRules::default());
        assert_eq!(GameplayRules::from_wire(&custom.wire()), Some(custom));
    }
}

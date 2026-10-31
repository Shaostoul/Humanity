//! What this device keeps about notifications (`config.json` `notifications`, AppConfig): the
//! choices that depend on the screen and the speakers, not on the person (docs/design/
//! notifications.md, section 7). The switches for which kinds notify and the quiet hours are the
//! person's and live on their server (`notification_prefs`), not here.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::data::Level;
use super::mutes::Mutes;

/// How much a notice says. Safe by default (operator, 2026-10-09: "Security is definitely
/// something we need to get right. We must protect the users."): who it is from, never what was
/// said, because a notice is seen by anyone near the screen and kept by the operating system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Show {
    /// Only that something arrived.
    Nothing,
    /// Who it is from and where (the default).
    #[default]
    Name,
    /// Who it is from and the start of the message: the person's own choice.
    NameAndText,
}

impl Show {
    /// Every choice, in the order Settings lists them.
    pub const ALL: [Show; 3] = [Show::Nothing, Show::Name, Show::NameAndText];

    /// The choice's id as `config.json` and the website write it.
    pub fn id(self) -> &'static str {
        match self {
            Show::Nothing => "nothing",
            Show::Name => "name",
            Show::NameAndText => "name_and_text",
        }
    }

    /// The choice for an id; anything unknown is the safe default.
    pub fn from_id(id: &str) -> Show {
        Show::ALL.into_iter().find(|s| s.id() == id).unwrap_or_default()
    }
}

/// This device's notification choices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalSettings {
    /// Notices from the operating system (the corner of the screen) while HumanityOS is behind.
    pub desktop: bool,
    /// What a notice shows.
    pub show: Show,
    /// A short sound with a notice.
    pub sound: bool,
    /// Which sound: an id from the data file's `sounds` (and `data/sounds.toml`).
    pub sound_id: String,
    /// Everything, servers, rooms and conversations muted, and until when.
    pub mutes: Mutes,
    /// Each room the person set to something other than mentions (rung 2), by
    /// `<server>|<room id>` (the room mutes' key). Absent: mentions.
    pub room_levels: BTreeMap<String, Level>,
}

impl Default for LocalSettings {
    fn default() -> Self {
        Self {
            desktop: true,
            show: Show::Name,
            sound: true,
            sound_id: "sfx.chat_message".into(),
            mutes: Mutes::default(),
            room_levels: BTreeMap::new(),
        }
    }
}

impl LocalSettings {
    /// What `room` on `server` is set to (mentions unless the person chose otherwise).
    pub fn room_level(&self, server: &str, room: &str) -> Level {
        self.room_levels.get(&Mutes::room_key(server, room)).copied().unwrap_or_default()
    }

    /// Set `room` on `server` to `level`; the default is kept as no entry, so the saved list holds
    /// only the rooms a person changed.
    pub fn set_room_level(&mut self, server: &str, room: &str, level: Level) {
        let key = Mutes::room_key(server, room);
        if level == Level::default() {
            self.room_levels.remove(&key);
        } else {
            self.room_levels.insert(key, level);
        }
    }
}

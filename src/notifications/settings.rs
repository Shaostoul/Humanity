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
    /// A short sound with a notice (the master switch: off, no kind makes a sound).
    pub sound: bool,
    /// Each kind's sound the person chose (rung 3), by kind id: an id from the data file's
    /// `sounds` (and `data/sounds.toml`), or [`NO_SOUND`]. A kind absent here plays its own
    /// default (the data file's `sound`). Replaced rung 1's single choice outright.
    pub sounds: BTreeMap<String, String>,
    /// Each game event the person turned on or off (rung 3), by event id; absent: the data file's
    /// `on`.
    pub events: BTreeMap<String, bool>,
    /// The always-allow list (rung 3, `notifications::allow`), by the identity it belongs to: the
    /// people whose calls and messages come through quiet hours.
    pub quiet_allow: BTreeMap<String, super::allow::QuietAllow>,
    /// Notes to oneself about the list made while no server was connected (their text and signed
    /// time), sent on the next connection (`engine/quiet_allow.rs`).
    pub quiet_allow_unsent: Vec<(String, u64)>,
    /// Everything, servers, rooms and conversations muted, and until when.
    pub mutes: Mutes,
    /// Each room the person set to something other than mentions (rung 2), by
    /// `<server>|<room id>` (the room mutes' key). Absent: mentions.
    pub room_levels: BTreeMap<String, Level>,
    /// The icon in the notification area, beside the clock (rung 2, part 2; on by default). Its
    /// right-click opens HumanityOS, mutes everything for a while, or quits.
    pub tray: bool,
    /// Closing the window hides it in the tray instead of quitting, so notices still arrive
    /// (rung 2, part 2). Off by default: closing quits unless the person turns this on, and it
    /// does nothing without the tray icon (there would be no way back to the window).
    pub close_to_tray: bool,
}

impl Default for LocalSettings {
    fn default() -> Self {
        Self {
            desktop: true,
            show: Show::Name,
            sound: true,
            sounds: BTreeMap::new(),
            events: BTreeMap::new(),
            quiet_allow: BTreeMap::new(),
            quiet_allow_unsent: Vec::new(),
            mutes: Mutes::default(),
            room_levels: BTreeMap::new(),
            tray: true,
            close_to_tray: false,
        }
    }
}

/// A kind's sound set to nothing (Settings' "None").
pub const NO_SOUND: &str = "none";

impl LocalSettings {
    /// The sound id kind `kind` plays: the person's choice, else the kind's default; `None` when it
    /// is set to [`NO_SOUND`] (or has no sound at all). Whether sound is on is [`Self::sound`].
    pub fn sound_of<'a>(&'a self, file: &'a super::data::File, kind: &str) -> Option<&'a str> {
        let chosen = self.sounds.get(kind).map(String::as_str).or_else(|| file.kind(kind).map(|k| k.sound.as_str()));
        chosen.filter(|s| !s.is_empty() && *s != NO_SOUND)
    }

    /// Set kind `kind`'s sound to `id` ([`NO_SOUND`] for none); its default is kept as no entry.
    pub fn set_sound(&mut self, file: &super::data::File, kind: &str, id: &str) {
        if file.kind(kind).is_some_and(|k| k.sound == id) {
            self.sounds.remove(kind);
        } else {
            self.sounds.insert(kind.to_string(), id.to_string());
        }
    }

    /// Is the game event `event` on (the person's choice, else the data file's default)?
    pub fn event_on(&self, file: &super::data::File, event: &str) -> bool {
        self.events.get(event).copied().unwrap_or_else(|| file.game_event(event).is_some_and(|e| e.on))
    }

    /// Turn the game event `event` on or off; its default is kept as no entry.
    pub fn set_event(&mut self, file: &super::data::File, event: &str, on: bool) {
        if file.game_event(event).is_some_and(|e| e.on == on) {
            self.events.remove(event);
        } else {
            self.events.insert(event.to_string(), on);
        }
    }

    /// The always-allow list of identity `me`.
    pub fn allow_list(&self, me: &str) -> Option<&super::allow::QuietAllow> {
        self.quiet_allow.get(me)
    }

    /// Is `key` on identity `me`'s always-allow list?
    pub fn always_allows(&self, me: &str, key: &str) -> bool {
        !me.is_empty() && self.allow_list(me).is_some_and(|l| l.allows(key))
    }

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

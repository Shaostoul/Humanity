//! `data/notifications/notifications.json`: every kind of notice, the levels a room can be set to,
//! the lengths something can be muted for, and the sounds a notice can play
//! (docs/design/notifications.md, section 2). The
//! website reads the same file (`web/shared/notify-rules.js`).
//!
//! Read once for the run through `load_text_or_embedded`, so a missing or damaged copy in the data
//! folder falls back to the one built into the exe. Every word is a key in `data/i18n/en.json`
//! (a lib test holds them to it).

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

const REL: &str = "notifications/notifications.json";

/// Which conversations a kind belongs to, so which mutes apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// A conversation with a person: no server or room mute applies.
    Person,
    /// A server's room: its server's and its own mutes apply.
    Room,
    /// The simulation.
    Game,
}

/// How much a room may notify (rung 2, docs/design/notifications.md section 5), the person's
/// choice per room from its right-click, kept on this device. Ordered: a room kind notifies only
/// in a room set to at least the kind's own `level`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// Nothing from this room notifies (its unread dot still shows).
    Nothing,
    /// Mentions and replies to you (every room's level until the person changes it).
    #[default]
    Mentions,
    /// Every line said in it.
    All,
}

impl Level {
    /// The id as `config.json`, the data file and the website write it.
    pub fn id(self) -> &'static str {
        match self {
            Level::Nothing => "nothing",
            Level::Mentions => "mentions",
            Level::All => "all",
        }
    }

    /// The level for an id; anything unknown is the default.
    pub fn from_id(id: &str) -> Level {
        [Level::Nothing, Level::Mentions, Level::All].into_iter().find(|l| l.id() == id).unwrap_or_default()
    }
}

/// Which synced quiet-hours exception lets a kind through quiet hours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuietException {
    /// From a friend (you follow each other), with "friends' direct messages" on.
    Friends,
    /// With "mentions" on.
    Mentions,
}

/// One kind of notice.
#[derive(Debug, Clone, Deserialize)]
pub struct Kind {
    pub id: String,
    /// Whether anything raises it yet.
    #[serde(default)]
    pub built: bool,
    /// The synced switch that turns it on or off: "dm", "mentions", "tasks", or none.
    #[serde(default)]
    pub server_flag: Option<String>,
    pub scope: Scope,
    /// A room kind: the least level its room must be set to (`None`: mentions).
    #[serde(default)]
    pub level: Option<Level>,
    /// The quiet-hours exception that lets it through quiet hours, if any.
    #[serde(default)]
    pub quiet_exception: Option<QuietException>,
    pub label: String,
    pub hint: String,
    /// The title naming the sender (`{{name}}`, `{{room}}`).
    pub title: String,
    /// The title when a notice shows nothing about the sender.
    pub title_hidden: String,
    /// The body when a notice does not show the message.
    pub body_hidden: String,
}

/// One length something can be muted for.
#[derive(Debug, Clone, Deserialize)]
pub struct MuteFor {
    pub id: String,
    /// 0: until the person unmutes.
    pub seconds: u64,
    pub label: String,
    pub hint: String,
}

/// One sound a notice can play.
#[derive(Debug, Clone, Deserialize)]
pub struct Sound {
    /// Its entry in `data/sounds.toml`.
    pub id: String,
    /// Used when that entry is missing.
    pub path: String,
    pub label: String,
}

/// One choice a room's right-click offers for its level.
#[derive(Debug, Clone, Deserialize)]
pub struct LevelChoice {
    pub id: Level,
    pub label: String,
    pub hint: String,
}

/// The whole file.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct File {
    #[serde(default)]
    pub kinds: Vec<Kind>,
    #[serde(default)]
    pub levels: Vec<LevelChoice>,
    #[serde(default)]
    pub mute_for: Vec<MuteFor>,
    #[serde(default)]
    pub sounds: Vec<Sound>,
}

impl File {
    pub fn parse(text: &str) -> Result<File, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }

    pub fn kind(&self, id: &str) -> Option<&Kind> {
        self.kinds.iter().find(|k| k.id == id)
    }

    /// The sound for `id`, or the first one when `id` is not in the file (a hand-edited config).
    pub fn sound(&self, id: &str) -> Option<&Sound> {
        self.sounds.iter().find(|s| s.id == id).or_else(|| self.sounds.first())
    }
}

/// The file, read once for the run. Empty (nothing notifies, Settings lists no choices) only when
/// both the data folder's copy and the exe's own fail, which a lib test rules out for the latter.
pub fn get() -> &'static File {
    static FILE: OnceLock<File> = OnceLock::new();
    FILE.get_or_init(|| {
        crate::embedded_data::load_text_or_embedded(&crate::data_dir(), REL, File::parse).unwrap_or_else(|e| {
            log::warn!("{e}: no notifications this run");
            File::default()
        })
    })
}

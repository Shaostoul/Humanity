//! The channels every server makes for itself (2026-11-02, docs/design/chat-channels.md section 4):
//! #announcements, #local, #global and #feedback, read from `data/server/default_channels.json`.
//!
//! WHY A DATA FILE (CLAUDE.md, infinite-of-x): the set used to be written in three places, #general
//! in `Storage::ensure_default_channel`, #announcements in relay/mod.rs's first-boot seed (and again
//! in scripts/seed-announcements.js), #local in the guaranteed-local-room block beside it. Adding
//! #feedback would have been a fourth. Now a default channel is a row of one file, and the code
//! here is the one place that makes them.
//!
//! WHY COMPILED IN (`include_str!`): the relay is built on GitHub and the program alone is swapped
//! in on the VPS; a server that follows releases (public.guide, scripts/relay-follow-releases.sh)
//! does check the repo out at the release, but a server run from the program alone would have no
//! `data/` beside it. The set is part of what a release does, so it travels inside the program, the
//! way `data/admin/actions.json` does (relay/admin/mod.rs). A server's admin changes their channels
//! in Server Settings, not in this file.
//!
//! WHEN EACH IS MADE: at every start (relay/mod.rs `run_relay`). A channel marked `always` is made
//! whenever it is missing: #global, which every server has, and #local while the guaranteed local
//! room is on (Server Settings > Federation) and no local-only channel is left. One marked `once` is
//! made the first time a server starts with it in the set, then recorded (`server_state`
//! `default_channel_made:<id>`), so a channel an admin deleted stays deleted (BUG-036): that is how
//! the live servers get #feedback at their next start. A channel already there keeps whatever its
//! admin set; only one made here gets the file's levels, place, voice and federation.

use humanity_protocol::chat_access::{PostLevel, ReadLevel};
use serde::Deserialize;

use crate::relay::storage::Storage;

/// The file, as built into the program.
const DEFAULT_CHANNELS_JSON: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/data/server/default_channels.json"));

/// When a default channel is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Made {
    /// Whenever it is missing, at every start.
    Always,
    /// The first time this server starts with it in the set; a deleted one stays deleted.
    Once,
}

/// One row of `data/server/default_channels.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct DefaultChannel {
    pub id: String,
    pub name: String,
    /// A word key in data/i18n/en.json, made into this server's words when the channel is made.
    pub description: String,
    pub read_level: String,
    pub post_level: String,
    pub position: i32,
    pub local_only: bool,
    pub federated: bool,
    pub voice: bool,
    pub made: Made,
}

impl DefaultChannel {
    pub fn read(&self) -> ReadLevel {
        ReadLevel::from_stored(&self.read_level)
    }

    pub fn post(&self) -> PostLevel {
        PostLevel::from_stored(&self.post_level)
    }

    /// The channel's description in this server's words: the English of data/i18n/en.json as built
    /// into the program. A description is the server's own, stored as words when the channel is
    /// made (an admin edits it in Server Settings), and the server build has no word lookup of its
    /// own (humanity_core::i18n is the desktop's); a key en.json lacks reads as itself.
    pub fn words(&self) -> String {
        static ENGLISH: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
        let english = ENGLISH.get_or_init(|| serde_json::from_str(humanity_core::embedded_data::I18N_EN_JSON).unwrap_or(serde_json::Value::Null));
        self.description.split('.').fold(english, |v, part| &v[part]).as_str().unwrap_or(&self.description).to_string()
    }
}

#[derive(Deserialize)]
struct File {
    channels: Vec<DefaultChannel>,
}

/// The default channels, in the file's order. The file is part of the program, and its test
/// (`default_channels_tests.rs`) parses it, so a file that does not parse is a build that fails
/// its tests, never a server that starts without channels.
pub fn all() -> &'static [DefaultChannel] {
    static ALL: std::sync::OnceLock<Vec<DefaultChannel>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| {
        serde_json::from_str::<File>(DEFAULT_CHANNELS_JSON)
            .expect("data/server/default_channels.json is built into the relay and parses (its test checks)")
            .channels
    })
}

/// The default channel `id`, if it is one.
pub fn get(id: &str) -> Option<&'static DefaultChannel> {
    all().iter().find(|c| c.id == id)
}

/// The `server_state` key that records a `once` channel as made on this server.
fn made_key(id: &str) -> String {
    format!("default_channel_made:{id}")
}

/// Make the default channels this server is missing (module docs say which and when), and return
/// the ids made. `local_room_wanted` is Server Settings' guaranteed local room.
pub fn make_missing(db: &Storage, local_room_wanted: bool) -> Vec<String> {
    let mut made = Vec::new();
    for ch in all() {
        let wanted = if ch.local_only {
            // The guarantee: a local-only room exists while it is on, whatever it is called now.
            local_room_wanted && !db.any_local_only_channel().unwrap_or(true)
        } else {
            match ch.made {
                Made::Always => !db.channel_exists(&ch.id).unwrap_or(true),
                Made::Once => db.get_state(&made_key(&ch.id)).ok().flatten().is_none(),
            }
        };
        if !wanted {
            continue;
        }
        if make(db, ch) {
            made.push(ch.id.clone());
        } else if ch.local_only && db.channel_exists(&ch.id).unwrap_or(false) {
            // The guarantee turned back on over a #local that became an ordinary channel while it
            // was off: that room is the local room again (as before 2026-11-02).
            let _ = db.set_channel_local_only(&ch.id, true);
            tracing::info!("Guaranteed local-only room: #{} is local only again", ch.id);
        }
        if ch.made == Made::Once {
            let _ = db.set_state(&made_key(&ch.id), "true");
        }
    }
    made
}

/// Make one default channel with the file's settings. False when it was already there (then
/// nothing about it is changed).
fn make(db: &Storage, ch: &DefaultChannel) -> bool {
    if !db.create_channel(&ch.id, &ch.name, Some(&ch.words()), "system", ch.post()).unwrap_or(false) {
        return false;
    }
    let _ = db.set_channel_access(&ch.id, ch.read(), ch.post());
    let _ = db.set_channel_position(&ch.id, ch.position);
    let _ = db.set_channel_voice_enabled(&ch.id, ch.voice);
    if ch.local_only {
        let _ = db.set_channel_local_only(&ch.id, true);
    } else {
        let _ = db.set_channel_federated(&ch.id, ch.federated);
    }
    tracing::info!("Default channel made: #{}", ch.id);
    true
}

#[cfg(test)]
#[path = "default_channels_tests.rs"]
mod tests;

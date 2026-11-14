//! Notifications: telling a person that something happened while they were looking elsewhere
//! (docs/design/notifications.md, the whole plan; rung 1 built 2026-10-30).
//!
//! THE RULE is one pure function, [`decide::decide`]: should this message notify, and how (a
//! notice from the operating system, a sound, both, or neither, and why). The website's twin is
//! `web/shared/notify-rules.js`; both answer the shared cases in
//! `scripts/tests/fixtures/notify-cases.json` the same way.
//!
//! - [`data`]: the kinds of notice, the mute lengths and the sounds, read from
//!   `data/notifications/notifications.json` (infinite-of-x: a new kind is a row there).
//! - [`settings`]: what this device keeps (`config.json` `notifications`): desktop notices on or
//!   off, what a notice shows, the sound, the mutes, and each room's level.
//! - [`mutes`]: everything, a server, a room, or a conversation, muted for a while or until unmuted.
//! - [`quiet`]: quiet hours (synced through the server's `notification_prefs`), by the device's clock.
//! - [`toast`]: the operating system's notice, on its own worker thread (Windows, Linux, macOS).
//! - [`list`]: the in-app list behind the header's bell (rung 2), in memory for the run only.
//! - [`tray`]: the notification-area icon's look, tooltip and menu (rung 2, part 2), pure.
//! - [`clicks`]: a click on a notice back to its conversation, and the banner and taskbar flash
//!   beside a notice (rung 2, part 2).
//! - [`allow`]: the always-allow list, the people whose calls and messages come through quiet
//!   hours, and the notes to oneself that carry it between devices (rung 3).
//! - [`game`]: game events folded, at most one notice of each kind per its interval (rung 3).
//!
//! The engine hands a live message to it in `src/engine/notify.rs`; nothing here runs per frame
//! except the bell reading the list's unseen count.

pub mod allow;
pub mod clicks;
pub mod data;
pub mod decide;
pub mod game;
pub mod list;
pub mod mutes;
pub mod quiet;
pub mod settings;
pub mod toast;
pub mod tray;

#[cfg(test)]
mod tests;

pub use data::Level;
pub use decide::{decide, is_mention, Candidate, Moment, Outcome, Synced, Why};
pub use mutes::Mutes;
pub use settings::{LocalSettings, Show};

/// What the notification system keeps for the run only (never saved): when each conversation last
/// had a notice (the 5-second burst guard), the in-app list, and what Settings asked for this frame.
#[derive(Debug, Default)]
pub struct Runtime {
    /// `decide::burst_key` (server and conversation) -> unix seconds of its last notice.
    /// reuse: when each conversation last gave a notice (the burst limit): bookkeeping, not built
    /// data.
    pub last_notice: std::collections::HashMap<String, i64>,
    /// Settings > Notifications > Show a test notification was pressed.
    pub test_requested: bool,
    /// Settings > Notifications > Play: the sound to play once, by its id in the data file.
    pub preview_sound: Option<String>,
    /// The in-app list (rung 2): the last notices, behind the header's bell. Never saved.
    pub list: list::List,
    /// The bell's panel is open.
    pub panel_open: bool,
    /// The run's notices and the conversations a click on one opens (rung 2, part 2).
    pub targets: clicks::Targets,
    /// The in-world banner showing now, if any (rung 2, part 2).
    pub banner: Option<clicks::Banner>,
    /// Game events waiting to become notices (rung 3).
    pub game: game::Folder,
    /// A call ringing with its notice up (rung 3): its sound repeats while it rings, and its notice
    /// is withdrawn when the ring ends however it ends (`engine/notify.rs` `pump_ringing`).
    pub ringing: Option<Ringing>,
}

/// A call whose notice is up (rung 3).
#[derive(Debug, Clone)]
pub struct Ringing {
    /// The caller's key.
    pub key: String,
    /// The notice's tag and group, to withdraw it.
    pub tag: String,
    pub group: String,
    /// Its sound, when it plays one (the kind's, `LocalSettings::sound_of`).
    pub sound: Option<String>,
    /// When the sound plays next, and when the ringing stops whatever happens.
    pub next_ring: std::time::Instant,
    pub until: std::time::Instant,
}

/// Seconds since the Unix epoch, now.
pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

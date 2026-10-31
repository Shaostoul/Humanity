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
//!   off, what a notice shows, the sound, and the mutes.
//! - [`mutes`]: everything, a server, or a room, muted for a while or until unmuted.
//! - [`quiet`]: quiet hours (synced through the server's `notification_prefs`), by the device's clock.
//! - [`toast`]: the operating system's notice, on its own worker thread (Windows, Linux, macOS).
//!
//! The engine hands a live message to it in `src/engine/notify.rs`; nothing here runs per frame.

pub mod data;
pub mod decide;
pub mod mutes;
pub mod quiet;
pub mod settings;
pub mod toast;

#[cfg(test)]
mod tests;

pub use decide::{decide, is_mention, Candidate, Moment, Outcome, Synced, Why};
pub use mutes::Mutes;
pub use settings::{LocalSettings, Show};

/// What the notification system keeps for the run only (never saved): when each conversation last
/// had a notice (the 5-second burst guard), and what Settings asked for this frame.
#[derive(Debug, Default)]
pub struct Runtime {
    /// Conversation key -> unix seconds of its last notice.
    pub last_notice: std::collections::HashMap<String, i64>,
    /// Settings > Notifications > Show a test notification was pressed.
    pub test_requested: bool,
    /// Settings > Notifications > Play: the sound to play once, by its id in the data file.
    pub preview_sound: Option<String>,
}

/// Seconds since the Unix epoch, now.
pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

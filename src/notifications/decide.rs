//! THE RULE: should this message notify, and how (docs/design/notifications.md, section 4).
//!
//! One pure function, so every case is a lib test and the website's twin
//! (`web/shared/notify-rules.js`) is held to the same cases
//! (`scripts/tests/fixtures/notify-cases.json`). The checks run in a fixed order and the first
//! that stops a notice names why ([`Why`]): a person's own message, a kind turned off, a mute,
//! quiet hours, the conversation already on screen, a burst. Only what passes them all can make a
//! sound or an operating-system notice, and the notice's words come from [`words`], which never
//! shows what was said unless the person chose that.

use super::data::{File, Scope};
use super::mutes::Mutes;
use super::settings::{LocalSettings, Show};

/// A notice for the same conversation within this many seconds of the last is held back: a burst
/// of lines is one notice, and a flood of mentions cannot make the computer a siren.
pub const BURST_SECS: i64 = 5;

/// Longest title and body a notice carries, in characters.
pub const TITLE_CHARS: usize = 80;
pub const BODY_CHARS: usize = 240;

/// A live message that might deserve a notice, built by the place that received it
/// (`src/engine/notify.rs` lists them).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Candidate {
    /// A kind's id in the data file ("dm", "mention").
    pub kind: String,
    /// From this person (this device or another of theirs).
    pub from_me: bool,
    /// The server it came through, normalised (`chat::norm_server_url`).
    pub server: String,
    /// A server's room it was said in; `None` for a conversation with a person.
    pub room: Option<String>,
    /// What the Chat page shows it under: "dm:<key>", or the room's id.
    pub conversation: String,
    /// Who wrote it, as the app shows them.
    pub sender: String,
    /// The room's name, for "in #room".
    pub room_name: String,
    /// What was said (already made safe to show: a file marker is its name, never its key).
    pub text: String,
}

/// The person's synced switches (the server's `notification_prefs`), as last received.
#[derive(Debug, Clone, Copy)]
pub struct Synced<'a> {
    pub dm: bool,
    pub mentions: bool,
    pub tasks: bool,
    pub quiet_start: Option<&'a str>,
    pub quiet_end: Option<&'a str>,
    /// The server's answer has arrived (the desktop asks at every sign-in). Until it has, quiet
    /// hours are unknown, so nothing notifies.
    pub loaded: bool,
}

impl Synced<'_> {
    /// Is the synced switch `flag` on? A kind with no switch is always on.
    pub fn flag(&self, flag: Option<&str>) -> bool {
        match flag {
            Some("dm") => self.dm,
            Some("mentions") => self.mentions,
            Some("tasks") => self.tasks,
            _ => true,
        }
    }
}

/// When it arrived, and what the person was looking at.
#[derive(Debug, Clone, Copy, Default)]
pub struct Moment {
    /// Unix seconds.
    pub now: i64,
    /// The device's local minute after midnight.
    pub minute: u16,
    /// The app's window has the keyboard focus.
    pub focused: bool,
    /// The conversation is the one showing (Chat page or in-world chat, same server).
    pub on_screen: bool,
    /// Unix seconds of this conversation's last notice, if any this run.
    pub last_notice: Option<i64>,
}

/// Why a message did or did not notify. The codes are what the shared cases and the website use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    Notify,
    OwnMessage,
    UnknownKind,
    NotLoaded,
    KindOff,
    AllMuted,
    ServerMuted,
    RoomMuted,
    QuietHours,
    OnScreen,
    TooSoon,
    NothingOn,
}

impl Why {
    pub fn code(self) -> &'static str {
        match self {
            Why::Notify => "notify",
            Why::OwnMessage => "own_message",
            Why::UnknownKind => "unknown_kind",
            Why::NotLoaded => "not_loaded",
            Why::KindOff => "kind_off",
            Why::AllMuted => "all_muted",
            Why::ServerMuted => "server_muted",
            Why::RoomMuted => "room_muted",
            Why::QuietHours => "quiet_hours",
            Why::OnScreen => "on_screen",
            Why::TooSoon => "too_soon",
            Why::NothingOn => "nothing_on",
        }
    }
}

/// The answer: why, and what to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    pub why: Why,
    /// Show the operating system's notice.
    pub toast: bool,
    /// Play the sound.
    pub sound: bool,
}

impl Outcome {
    fn quiet(why: Why) -> Outcome {
        Outcome { why, toast: false, sound: false }
    }
}

/// THE RULE. Pure: everything it reads is an argument.
pub fn decide(file: &File, c: &Candidate, synced: &Synced, local: &LocalSettings, at: &Moment) -> Outcome {
    if c.from_me {
        return Outcome::quiet(Why::OwnMessage);
    }
    let Some(kind) = file.kind(&c.kind) else {
        return Outcome::quiet(Why::UnknownKind);
    };
    if !synced.loaded {
        return Outcome::quiet(Why::NotLoaded);
    }
    if !synced.flag(kind.server_flag.as_deref()) {
        return Outcome::quiet(Why::KindOff);
    }
    if let Some(why) = muted(&local.mutes, kind.scope, c, at.now) {
        return Outcome::quiet(why);
    }
    if let Some((start, end)) = super::quiet::window(synced.quiet_start, synced.quiet_end) {
        if super::quiet::in_quiet(start, end, at.minute) {
            return Outcome::quiet(Why::QuietHours);
        }
    }
    if at.focused && at.on_screen {
        return Outcome::quiet(Why::OnScreen);
    }
    if at.last_notice.is_some_and(|t| at.now - t < BURST_SECS && at.now >= t) {
        return Outcome::quiet(Why::TooSoon);
    }
    let toast = local.desktop && !at.focused;
    let sound = local.sound;
    if !toast && !sound {
        return Outcome::quiet(Why::NothingOn);
    }
    Outcome { why: Why::Notify, toast, sound }
}

/// The mute that stops `c`, if any: everything first, then (for a room) its server, then the room.
fn muted(m: &Mutes, scope: Scope, c: &Candidate, now: i64) -> Option<Why> {
    if m.everything_muted(now).is_some() {
        return Some(Why::AllMuted);
    }
    if scope != Scope::Room {
        return None;
    }
    if m.server_muted(&c.server, now).is_some() {
        return Some(Why::ServerMuted);
    }
    match &c.room {
        Some(room) if m.room_muted(&c.server, room, now).is_some() => Some(Why::RoomMuted),
        _ => None,
    }
}

/// Is `text` a mention of `name`: `@` and the name, any case, not followed by a letter, a digit,
/// `_` or `-` (so `@Al` is not `@Alice`)? An empty name is never mentioned.
pub fn is_mention(text: &str, name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() {
        return false;
    }
    let hay: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
    let needle: Vec<char> = std::iter::once('@').chain(name.chars().flat_map(char::to_lowercase)).collect();
    if needle.len() > hay.len() {
        return false;
    }
    (0..=hay.len() - needle.len()).any(|i| {
        hay[i..i + needle.len()] == needle[..]
            && hay.get(i + needle.len()).is_none_or(|&ch| !(ch.is_alphanumeric() || ch == '_' || ch == '-'))
    })
}

/// A notice's title and body for `c`, as `show` allows: nothing about the sender, who and where,
/// or who and the start of the message. The words are the kind's keys in `data/i18n/`.
pub fn words(file: &File, c: &Candidate, show: Show) -> (String, String) {
    let Some(kind) = file.kind(&c.kind) else {
        return (String::new(), String::new());
    };
    let t = crate::gui::i18n::t_dyn;
    let name = clean(&c.sender, TITLE_CHARS);
    let room = clean(&c.room_name, TITLE_CHARS);
    let named = || clean(&crate::gui::i18n::fill(t(&kind.title), &[("name", &name), ("room", &room)]), TITLE_CHARS);
    match show {
        Show::Nothing => (t(&kind.title_hidden).to_string(), t(&kind.body_hidden).to_string()),
        Show::Name => (named(), t(&kind.body_hidden).to_string()),
        Show::NameAndText => {
            let body = clean(&c.text, BODY_CHARS);
            let body = if body.is_empty() { t(&kind.body_hidden).to_string() } else { body };
            (named(), body)
        }
    }
}

/// `s` made fit for a notice: every control character (a new line too) a space, runs of spaces
/// one, trimmed, and cut to `max` characters with an ellipsis. Never cuts inside a character.
pub fn clean(s: &str, max: usize) -> String {
    let mut out = String::with_capacity(s.len().min(max * 4));
    let mut space = false;
    for ch in s.chars() {
        let ch = if ch.is_control() || ch.is_whitespace() { ' ' } else { ch };
        if ch == ' ' {
            if space || out.is_empty() {
                continue;
            }
            space = true;
        } else {
            space = false;
        }
        out.push(ch);
    }
    let out = out.trim_end();
    if out.chars().count() <= max {
        return out.to_string();
    }
    let cut: String = out.chars().take(max.saturating_sub(1)).collect();
    format!("{}\u{2026}", cut.trim_end())
}

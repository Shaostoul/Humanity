//! THE RULE: should this message notify, and how (docs/design/notifications.md, section 4).
//!
//! One pure function, so every case is a lib test and the website's twin
//! (`web/shared/notify-rules.js`) is held to the same cases
//! (`scripts/tests/fixtures/notify-cases.json`). The checks run in a fixed order and the first
//! that stops a notice names why ([`Why`]): a person's own message, a kind turned off, a mute, the
//! room's level, quiet hours (unless an exception lets it through), the conversation already on
//! screen, a burst. Only what passes them all can make a sound, an operating-system notice or an
//! entry in the in-app list (rung 2), and the notice's words come from [`words`], which never
//! shows what was said unless the person chose that.

use super::data::{File, QuietException, Scope};
use super::mutes::Mutes;
use super::settings::{LocalSettings, Show};

/// A notice for the same conversation within this many seconds of the last is held back: a burst
/// of lines is one notice, and a flood of mentions cannot make the computer a siren.
pub const BURST_SECS: i64 = 5;

/// The burst guard's key for `c` (and the in-app list's, `list::List::add`): its server and its
/// conversation, so two servers' rooms of one name (each has a "general") keep their own five
/// seconds (rung 2, part 2: rung 2's first part keyed by the conversation alone); but a Commons
/// room (`commons`: carried by two or more of my linked servers) is `commons:<room>`, ONE room
/// whichever carrier delivers a copy of its line, so a mention bridged between two servers is one
/// notice, one sound and one entry, not two (the critic's review of part 2, 2026-10-31).
pub fn burst_key(c: &Candidate) -> String {
    key_of(&c.server, &c.conversation, c.room.as_deref(), c.commons)
}

/// [`burst_key`] from its parts (the in-app list keys its entries with it).
pub fn key_of(server: &str, conversation: &str, room: Option<&str>, commons: bool) -> String {
    match (room, commons) {
        (Some(room), true) => format!("commons:{room}"),
        _ => format!("{server}|{conversation}"),
    }
}

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
    /// From a friend: the person and the sender follow each other (the client-held store, whose
    /// follows travel with friendship passes; `DmStore::is_friend`). Decided on the device: the
    /// server cannot know it for a DM (sealed sender).
    pub from_friend: bool,
    /// A Commons room: `room` on this server is one room with another of my linked servers
    /// (`chat::in_commons`), so its notices are keyed by the room alone ([`burst_key`]).
    pub commons: bool,
    /// The sender is on the person's always-allow list (rung 3, `notifications::allow`): their
    /// calls and messages (the person kinds) come through quiet hours. Decided on the device; no
    /// server knows the list.
    pub from_allowed: bool,
    /// A game event's id in the data file's `game_events` (rung 3); empty for every other kind.
    pub event: String,
    /// How many events a folded game notice stands for (rung 3, `notifications::game`).
    pub count: u32,
    /// Which of the kind's titles (rung 3: a task newly given to you is `assigned`); empty: the
    /// kind's own.
    pub variant: String,
}

/// The person's synced switches (the server's `notification_prefs`), as last received.
#[derive(Debug, Clone, Copy)]
pub struct Synced<'a> {
    pub dm: bool,
    pub mentions: bool,
    pub tasks: bool,
    pub quiet_start: Option<&'a str>,
    pub quiet_end: Option<&'a str>,
    /// Quiet-hours exception: a friend's direct message still notifies (rung 2, off by default).
    pub quiet_friends: bool,
    /// Quiet-hours exception: a mention or a reply still notifies (rung 2, off by default).
    pub quiet_mentions: bool,
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

    /// Does the synced exception `e` let a notice from a friend (or not) through quiet hours?
    pub fn excepts(&self, e: Option<QuietException>, from_friend: bool) -> bool {
        match e {
            Some(QuietException::Friends) => self.quiet_friends && from_friend,
            Some(QuietException::Mentions) => self.quiet_mentions,
            None => false,
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
    /// A game event the person turned off on this device, or one off by default and not turned on
    /// (rung 3).
    EventOff,
    AllMuted,
    /// The conversation with that person is muted (rung 2).
    PersonMuted,
    ServerMuted,
    RoomMuted,
    /// The room is set below what the kind needs: an ordinary line in a room at mentions, or
    /// anything in a room set to nothing (rung 2).
    RoomLevel,
    QuietHours,
    OnScreen,
    TooSoon,
}

impl Why {
    pub fn code(self) -> &'static str {
        match self {
            Why::Notify => "notify",
            Why::OwnMessage => "own_message",
            Why::UnknownKind => "unknown_kind",
            Why::NotLoaded => "not_loaded",
            Why::KindOff => "kind_off",
            Why::EventOff => "event_off",
            Why::AllMuted => "all_muted",
            Why::PersonMuted => "person_muted",
            Why::ServerMuted => "server_muted",
            Why::RoomMuted => "room_muted",
            Why::RoomLevel => "room_level",
            Why::QuietHours => "quiet_hours",
            Why::OnScreen => "on_screen",
            Why::TooSoon => "too_soon",
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
    /// Put an entry in the in-app list (the header's bell, rung 2). Whatever passes every check
    /// is listed, also with the window in front and with desktop notices and sound both off: the
    /// list is the one surface that never interrupts. What a check stops is not listed: a line
    /// on screen needs no entry, and a line inside the burst guard belongs to the entry its
    /// burst already made (`list::List` also folds a conversation's later notices into its entry
    /// until the panel is opened).
    pub list: bool,
}

impl Outcome {
    fn quiet(why: Why) -> Outcome {
        Outcome { why, toast: false, sound: false, list: false }
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
    if kind.scope == Scope::Game {
        // Each event is its own switch on this device (rung 3); one the file lacks is unknown.
        if file.game_event(&c.event).is_none() {
            return Outcome::quiet(Why::UnknownKind);
        }
        if !local.event_on(file, &c.event) {
            return Outcome::quiet(Why::EventOff);
        }
    }
    if let Some(why) = muted(&local.mutes, kind.scope, c, at.now) {
        return Outcome::quiet(why);
    }
    if kind.scope == Scope::Room {
        // Every room kind needs at least mentions (a lib test holds the data file to it), so a
        // room set to nothing never notifies.
        let set = c.room.as_deref().map(|r| local.room_level(&c.server, r)).unwrap_or_default();
        if set < kind.level.unwrap_or_default() {
            return Outcome::quiet(Why::RoomLevel);
        }
    }
    if let Some((start, end)) = super::quiet::window(synced.quiet_start, synced.quiet_end) {
        // The always-allow list (rung 3, the operator's decision) lets a person's calls and
        // messages through: the person kinds, never a room's line.
        let allowed = kind.scope == Scope::Person && c.from_allowed;
        if super::quiet::in_quiet(start, end, at.minute) && !synced.excepts(kind.quiet_exception, c.from_friend) && !allowed {
            return Outcome::quiet(Why::QuietHours);
        }
    }
    if at.focused && at.on_screen {
        return Outcome::quiet(Why::OnScreen);
    }
    if kind.burst && at.last_notice.is_some_and(|t| at.now - t < BURST_SECS && at.now >= t) {
        return Outcome::quiet(Why::TooSoon);
    }
    // The sound: on, and the kind not set to none (rung 3, per-kind sound).
    let sound = local.sound && local.sound_of(file, &kind.id).is_some();
    Outcome { why: Why::Notify, toast: local.desktop && !at.focused, sound, list: true }
}

/// The mute that stops `c`, if any: everything first, then a person's conversation, or (for a
/// room) its server, then the room.
fn muted(m: &Mutes, scope: Scope, c: &Candidate, now: i64) -> Option<Why> {
    if m.everything_muted(now).is_some() {
        return Some(Why::AllMuted);
    }
    match scope {
        Scope::Person if m.person_muted(&c.conversation, now).is_some() => Some(Why::PersonMuted),
        Scope::Room | Scope::Server if m.server_muted(&c.server, now).is_some() => Some(Why::ServerMuted),
        Scope::Room => match &c.room {
            Some(room) if m.room_muted(&c.server, room, now).is_some() => Some(Why::RoomMuted),
            _ => None,
        },
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
    let t = humanity_core::i18n::t_dyn;
    // A game event (rung 3): the event's own words, unless the notice shows nothing.
    if let (Some(e), false) = (file.game_event(&c.event), show == Show::Nothing) {
        let count = c.count.max(1) as i64;
        let place = clean(&c.room_name, TITLE_CHARS);
        let body = humanity_core::i18n::fill(t(&format!("{}.{}", e.body, if count == 1 { "one" } else { "other" })), &[("n", &count), ("where", &place)]);
        return (clean(t(&e.title), TITLE_CHARS), clean(&body, BODY_CHARS));
    }
    let name = clean(&c.sender, TITLE_CHARS);
    let room = clean(&c.room_name, TITLE_CHARS);
    let title_key = kind.variants.get(&c.variant).unwrap_or(&kind.title);
    let named = || clean(&humanity_core::i18n::fill(t(title_key), &[("name", &name), ("room", &room)]), TITLE_CHARS);
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

//! The always-allow list (rung 3, 2026-11-01; docs/design/notifications.md section 11): the people
//! whose calls and direct messages come through the person's quiet hours. The operator, verbatim:
//! "I like the default being calls don't go through on quiet hours unless I specifically add
//! users to the always allow list."
//!
//! - **Nobody is on it until the person adds them** (a conversation's menu, "Always allow during
//!   quiet hours"), and it names people by their identity key, the same on every server.
//! - **Client-held, never on a server:** who may wake you is a social edge, so it travels between
//!   the person's own devices as notes to oneself, the way the block list does
//!   (`net/block_list.rs`): a sealed DM from me to me whose text is exactly
//!   `[[hum:quiet-allow:v1]]<key>/on` or `/off` ([`note_text`], `CTL_QUIET_ALLOW`; the website
//!   builds the same bytes, `web/shared/notify-rules.js`, and both hold the test vector).
//! - **The newest note wins, per person** ([`QuietAllow::apply`], by the note's signed time), so a
//!   note delivered again, or two devices' changes arriving out of order, settle the same on every
//!   device. A person taken off keeps their entry (off, with its time), so an older "on" arriving
//!   late cannot put them back.
//!
//! Pure: no socket, no GUI. The engine's half (sending and taking the notes) is
//! `src/engine/quiet_allow.rs`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The note's marker (must match the website exactly; `net::dm_pq::CTL_QUIET_ALLOW`).
pub const MARKER: &str = crate::net::dm_pq::CTL_QUIET_ALLOW;

/// The longest key a note may carry. A Dilithium3 key in hex is 3,904 characters.
const MAX_KEY_CHARS: usize = 8_192;

/// One person's place on the list: on or off, and the signed time of the note that said so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub on: bool,
    /// Milliseconds since the epoch, the note's own signed time.
    pub at: u64,
}

/// One identity's list.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct QuietAllow {
    /// Identity key (lower-case hex) -> its newest entry.
    pub people: BTreeMap<String, Entry>,
}

impl QuietAllow {
    /// Is `key` always allowed?
    pub fn allows(&self, key: &str) -> bool {
        self.people.get(&key.to_ascii_lowercase()).is_some_and(|e| e.on)
    }

    /// Everyone on the list now, in key order.
    pub fn allowed(&self) -> impl Iterator<Item = &str> {
        self.people.iter().filter(|(_, e)| e.on).map(|(k, _)| k.as_str())
    }

    /// Take a note saying `key` is `on` (or not) as of `at`. True when it changed the list: only a
    /// note newer than what is held for that person counts.
    pub fn apply(&mut self, key: &str, on: bool, at: u64) -> bool {
        let key = key.to_ascii_lowercase();
        match self.people.get(&key) {
            Some(e) if e.at >= at => false,
            Some(e) if e.on == on => {
                self.people.insert(key, Entry { on, at });
                false
            }
            _ => {
                self.people.insert(key, Entry { on, at });
                true
            }
        }
    }
}

/// The note's text for `key` on or off: the marker, the key in lower-case hex, `/`, `on` or `off`.
pub fn note_text(key: &str, on: bool) -> String {
    format!("{MARKER}{}/{}", key.to_ascii_lowercase(), if on { "on" } else { "off" })
}

/// Read a note: `(key, on)`, or `None` for a text that is not a usable one (no key, a key that is
/// not lower-case hex, anything but `on` or `off`). A text that starts with the marker is a note
/// for [`is_note`] either way, so a malformed one is dropped, never shown as a message.
pub fn parse(text: &str) -> Option<(String, bool)> {
    let (key, state) = text.strip_prefix(MARKER)?.split_once('/')?;
    let key_ok = !key.is_empty() && key.len() <= MAX_KEY_CHARS && key.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if !key_ok {
        return None;
    }
    match state {
        "on" => Some((key.to_string(), true)),
        "off" => Some((key.to_string(), false)),
        _ => None,
    }
}

/// Does this DM text start with the marker? Such a text is never shown as a message.
pub fn is_note(text: &str) -> bool {
    text.starts_with(MARKER)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE NOTE'S BYTES, against the vector both apps hold: `ab12` on gives exactly
    /// `[[hum:quiet-allow:v1]]ab12/on`, which reads back; malformed notes are not usable but are
    /// still never shown. Seen red 2026-11-01 with the key left in the case it was given:
    /// "AB12" made "[[hum:quiet-allow:v1]]AB12/on", which the parse then refused.
    #[test]
    fn the_note_has_the_agreed_bytes_and_reads_back() {
        assert_eq!(note_text("AB12", true), "[[hum:quiet-allow:v1]]ab12/on", "the vector both apps hold");
        assert_eq!(note_text("ab12", false), "[[hum:quiet-allow:v1]]ab12/off");
        assert_eq!(parse(&note_text("ab12", true)), Some(("ab12".into(), true)));
        assert_eq!(parse("[[hum:quiet-allow:v1]]ab12/off"), Some(("ab12".into(), false)));
        for bad in ["[[hum:quiet-allow:v1]]ab12/", "[[hum:quiet-allow:v1]]/on", "[[hum:quiet-allow:v1]]ab 12/on", "[[hum:quiet-allow:v1]]AB12/on", "[[hum:quiet-allow:v1]]ab12/yes", "[[hum:quiet-allow:v1]]ab12"] {
            assert_eq!(parse(bad), None, "not usable: {bad}");
            assert!(is_note(bad), "but never shown: {bad}");
        }
        assert!(!is_note("I said [[hum:quiet-allow:v1]]ab12/on"), "the marker must lead");
    }

    /// THE NEWEST NOTE WINS, per person: a note delivered again changes nothing, an older "on"
    /// arriving after a newer "off" does not put the person back, and two devices' notes settle the
    /// same in either order. Seen red 2026-11-01 with `apply` taking every note in arrival order:
    /// "an older on after a newer off" put Ann back on the list.
    #[test]
    fn the_newest_note_wins_whatever_the_order() {
        let mut a = QuietAllow::default();
        assert!(!a.allows("ann"), "nobody until added");
        assert!(a.apply("ann", true, 100));
        assert!(a.allows("ANN"), "the key any case");
        assert!(!a.apply("ann", true, 100), "the same note again");
        assert!(a.apply("ann", false, 200));
        assert!(!a.apply("ann", true, 150), "an older on after a newer off");
        assert!(!a.allows("ann"));
        let mut b = QuietAllow::default();
        b.apply("ann", false, 200);
        b.apply("ann", true, 100);
        assert_eq!(a, b, "either order, the same list");
        a.apply("bo", true, 300);
        assert_eq!(a.allowed().collect::<Vec<_>>(), vec!["bo"]);
    }
}

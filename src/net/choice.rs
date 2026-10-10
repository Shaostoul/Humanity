//! The choice for each friend, as a note to oneself (section 10n of
//! docs/design/blocking-and-safe-mode.md, 2026-10-10): the words of the note, with no socket and no
//! GUI, so each rule has a unit test. The half that acts on the app is src/engine/choice.rs; the
//! stored choice is the DM store's (net/dm_store.rs `choices`).
//!
//! WHY A NOTE. Three reviews in one day found ways a choice made on one of a person's devices was
//! lost or reversed on another, all from one root: every device rebuilt the ticks from echoes of
//! passes, and echoes arrive late, out of order, or never. So the choice travels on its own, the
//! way the block list does (net/block_list.rs): a DM from me to me, sealed to my own key with the
//! ordinary signed inner, whose text is exactly `[[hum:choice:v1]]<friend key>/<may>`. The key is
//! the friend's identity key in lower-case hex; the `may` is the canonical one a pass carries
//! (sorted, comma-joined, `invite` alone when nothing is ticked). The signed time of the DM is
//! when the choice was made, which decides which of two choices is newer.
//!
//! The web client builds the same bytes (web/chat/crypto.js), and both hold the test vector below.

use super::dm_pq::CTL_CHOICE;
use crate::relay::core::pq_crypto::FriendMay;

/// The longest key a note may carry. A Dilithium3 key in hex is 3,904 characters.
const MAX_KEY_CHARS: usize = 8_192;

/// A note to oneself saying what one friend may do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceNote {
    /// The friend's identity key, lower-case hex.
    pub key: String,
    /// The canonical `may` their pass should carry.
    pub may: String,
}

impl ChoiceNote {
    pub fn new(key: &str, may: &str) -> Self {
        Self { key: key.to_string(), may: may.to_string() }
    }

    /// The note's text: the marker, the key, a `/`, the `may`, nothing between.
    pub fn text(&self) -> String {
        format!("{CTL_CHOICE}{}/{}", self.key, self.may)
    }

    /// Read a note, or None when the text is not a usable one: no key, a key that is not
    /// lower-case hex, no `may`, or a `may` that is not a pass's own canonical words (a word outside
    /// the pass's five, a repeat, or out of order). A text that starts with the marker is still a
    /// note for `is_note`, so a malformed one is dropped rather than shown as a message.
    pub fn parse(text: &str) -> Option<Self> {
        let (key, may) = text.strip_prefix(CTL_CHOICE)?.split_once('/')?;
        let key_ok = !key.is_empty() && key.len() <= MAX_KEY_CHARS && key.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !key_ok {
            return None;
        }
        let may = FriendMay::parse(may).ok()?.wire();
        Some(Self { key: key.to_string(), may })
    }

    /// Does this DM text start with the marker? Such a text is never shown as a message, readable
    /// or not.
    pub fn is_note(text: &str) -> bool {
        text.starts_with(CTL_CHOICE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE NOTE'S BYTES AND ITS PARSE, against the vector both clients hold (10n): friend `ab12`,
    /// may `invite,message,trade,voice_message` gives exactly
    /// `[[hum:choice:v1]]ab12/invite,message,trade,voice_message`, which reads back the same; a note
    /// with no may, no key, a key with a space, or a may word outside the pass's five is not
    /// usable, yet is still never shown as a message.
    /// Seen red 2026-10-10 with `CTL_CHOICE` spelt "[[hum:choice:v2]]" in net/dm_pq.rs: "the vector
    /// both clients hold" failed (left "[[hum:choice:v2]]ab12/...").
    #[test]
    fn the_note_has_the_agreed_bytes_and_reads_back() {
        let note = ChoiceNote::new("ab12", "invite,message,trade,voice_message");
        assert_eq!(note.text(), "[[hum:choice:v1]]ab12/invite,message,trade,voice_message", "the vector both clients hold");
        assert_eq!(ChoiceNote::parse(&note.text()), Some(note.clone()), "it reads back");
        assert!(ChoiceNote::is_note(&note.text()));
        let all_off = ChoiceNote::new(&"0f1e".repeat(976), "invite");
        assert_eq!(ChoiceNote::parse(&all_off.text()), Some(all_off), "a whole key, nothing ticked");

        for bad in [
            "[[hum:choice:v1]]ab12/",
            "[[hum:choice:v1]]/message",
            "[[hum:choice:v1]]ab 12/message",
            "[[hum:choice:v1]]ab12/message,fly",
            "[[hum:choice:v1]]ab12",
            "[[hum:choice:v1]]AB12/message",
            "[[hum:choice:v1]]ab12/trade,message",
        ] {
            assert_eq!(ChoiceNote::parse(bad), None, "not a usable note: {bad}");
            assert!(ChoiceNote::is_note(bad), "but never shown as a message: {bad}");
        }
        assert_eq!(ChoiceNote::parse(" [[hum:choice:v1]]ab12/message"), None, "the marker must lead");
        assert!(!ChoiceNote::is_note("I said [[hum:choice:v1]]ab12/message"));
    }
}
